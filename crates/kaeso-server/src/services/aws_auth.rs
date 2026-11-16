//! AWS credentials for Amazon Bedrock keys that sign their requests with Signature Version 4.
//!
//! - `aws_access_key`: an access key id, a secret access key and an optional session token, kept
//!   sealed as one JSON secret.
//! - `aws_role`: the gateway's own AWS identity, found the way AWS's SDKs find it
//!   (https://docs.aws.amazon.com/sdkref/latest/guide/standardized-credentials.html): the `AWS_*`
//!   environment variables, a web identity token file (EKS IRSA) through STS
//!   `AssumeRoleWithWebIdentity`, the ECS or EKS container credentials endpoint, then EC2's
//!   instance metadata (IMDSv2). The key can name a role to assume from there through STS
//!   `AssumeRole`, with an external id.
//!
//! Temporary credentials are cached per key and renewed five minutes before they expire.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;

use kaeso_core::provider::AuthMethod;
use kaeso_kpp::KppError;
use kaeso_upstream::sigv4::{AwsCredentials, Request as Signing, SigV4};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::services::credentials::Resolved;
use crate::state::AppState;

/// The service name Bedrock's runtime signs with.
pub const BEDROCK: &str = "bedrock";

/// Renew temporary credentials this long before they expire.
const RENEW_BEFORE: time::Duration = time::Duration::minutes(5);
const TIMEOUT: Duration = Duration::from_secs(5);
const ECS_HOST: &str = "http://169.254.170.2";
const IMDS: &str = "http://169.254.169.254";
const SESSION_NAME: &str = "kaeso";

/// An access key as a key keeps it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessKey {
    pub access_key_id: String,
    pub secret_access_key: String,
    #[serde(default)]
    pub session_token: Option<String>,
}

/// The role a key that signs in as the gateway's identity assumes, if any.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    #[serde(default)]
    pub role_arn: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    NotJson,
    Missing(&'static str),
    Malformed(&'static str),
}

impl AccessKey {
    pub fn parse(json: &str) -> Result<AccessKey, Problem> {
        let v: serde_json::Value = serde_json::from_str(json).map_err(|_| Problem::NotJson)?;
        for field in ["access_key_id", "secret_access_key"] {
            if v.get(field)
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
            {
                return Err(Problem::Missing(field));
            }
        }
        let key: AccessKey = serde_json::from_value(v).map_err(|_| Problem::NotJson)?;
        // AKIA… for a long-term key, ASIA… for a temporary one: 16 to 128 letters and digits.
        if !(16..=128).contains(&key.access_key_id.len())
            || !key.access_key_id.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            return Err(Problem::Malformed("access_key_id"));
        }
        if key.secret_access_key.len() > 128 || key.secret_access_key.contains(char::is_whitespace)
        {
            return Err(Problem::Malformed("secret_access_key"));
        }
        if key
            .session_token
            .as_deref()
            .is_some_and(|t| t.is_empty() || t.len() > 8192 || t.contains(char::is_whitespace))
        {
            return Err(Problem::Malformed("session_token"));
        }
        Ok(key)
    }

    pub fn last4(&self) -> String {
        let id = &self.access_key_id;
        id[id.len().saturating_sub(4)..].to_owned()
    }

    fn credentials(&self) -> AwsCredentials {
        AwsCredentials {
            access_key_id: self.access_key_id.clone(),
            secret_access_key: SecretString::from(self.secret_access_key.clone()),
            session_token: self.session_token.clone().map(SecretString::from),
        }
    }
}

impl Role {
    pub fn parse(json: &str) -> Result<Role, Problem> {
        let json = json.trim();
        let role: Role = if json.is_empty() {
            Role::default()
        } else {
            serde_json::from_str(json).map_err(|_| Problem::NotJson)?
        };
        if let Some(arn) = &role.role_arn
            && !is_role_arn(arn)
        {
            return Err(Problem::Malformed("role_arn"));
        }
        if let Some(id) = &role.external_id
            && (!(2..=1224).contains(&id.len())
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"+=,.@:/-_".contains(&b)))
        {
            return Err(Problem::Malformed("external_id"));
        }
        Ok(role)
    }

    /// What the key signs in as, for the list: the role's name, or the gateway's own identity.
    pub fn label(&self) -> String {
        self.role_arn
            .as_deref()
            .and_then(|a| a.rsplit('/').next())
            .unwrap_or("gateway")
            .to_owned()
    }
}

// arn:<partition>:iam::<12 digits>:role/<path and name>.
fn is_role_arn(arn: &str) -> bool {
    let parts: Vec<&str> = arn.splitn(6, ':').collect();
    matches!(parts.as_slice(), ["arn", partition, "iam", "", account, resource]
        if partition.starts_with("aws")
            && account.len() == 12
            && account.bytes().all(|b| b.is_ascii_digit())
            && resource.strip_prefix("role/").is_some_and(|r| !r.is_empty() && r.len() <= 512))
}

/// How a Bedrock request of `credential` signs itself, in `region`.
pub async fn signer(
    state: &AppState,
    credential: &Resolved,
    region: &str,
) -> Result<SigV4, KppError> {
    let credentials = credentials(state, credential, region).await?;
    Ok(SigV4 {
        credentials,
        region: region.to_owned(),
        service: BEDROCK.to_owned(),
        at: state.clock.now(),
    })
}

/// The credentials of a key that signs with SigV4.
pub async fn credentials(
    state: &AppState,
    credential: &Resolved,
    region: &str,
) -> Result<AwsCredentials, KppError> {
    let secret = credential.secret.expose_secret();
    match credential.method {
        AuthMethod::AwsAccessKey => AccessKey::parse(secret)
            .map(|k| k.credentials())
            .map_err(|_| KppError::provider("The AWS access key can't be read; replace it.")),
        AuthMethod::AwsRole => {
            if !state.config.aws.ambient_credentials {
                return Err(KppError::provider(
                    "This install doesn't let keys sign in as the gateway's AWS identity; an instance admin can allow it with `aws.ambient_credentials`.",
                ));
            }
            let role = Role::parse(secret)
                .map_err(|_| KppError::provider("The key's AWS role can't be read; edit it."))?;
            let now = state.clock.now();
            let cache_key = (credential.id.uuid(), credential.revision);
            if let Some(c) = cached(&cache_key, now) {
                return Ok(c);
            }
            let (creds, expires) = role_credentials(&role, region, now)
                .await
                .map_err(|e| KppError::provider(format!("Kaeso can't get AWS credentials: {e}")))?;
            remember(cache_key, creds.clone(), expires);
            Ok(creds)
        }
        _ => Err(KppError::provider(
            "The key doesn't sign its requests with AWS credentials.",
        )),
    }
}

/// Drops the cached credentials of a key, as after AWS answered that they expired.
pub fn forget(credential: &Resolved) {
    if let Ok(mut c) = CACHE.write() {
        c.remove(&(credential.id.uuid(), credential.revision));
    }
}

type CacheKey = (uuid::Uuid, i64);

/// Credentials and when they expire, if they do.
type Expiring = (AwsCredentials, Option<OffsetDateTime>);

static CACHE: LazyLock<RwLock<HashMap<CacheKey, Expiring>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn cached(key: &CacheKey, now: OffsetDateTime) -> Option<AwsCredentials> {
    let cache = CACHE.read().ok()?;
    let (creds, expires) = cache.get(key)?;
    expires
        .is_none_or(|e| now + RENEW_BEFORE < e)
        .then(|| creds.clone())
}

fn remember(key: CacheKey, creds: AwsCredentials, expires: Option<OffsetDateTime>) {
    if let Ok(mut c) = CACHE.write() {
        c.insert(key, (creds, expires));
    }
}

/// Where the gateway's own identity is looked up: the process environment, which tests replace.
pub trait Environment: Send + Sync {
    fn var(&self, name: &str) -> Option<String>;
}

struct Process;

impl Environment for Process {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.is_empty())
    }
}

static ENVIRONMENT: LazyLock<RwLock<Arc<dyn Environment>>> =
    LazyLock::new(|| RwLock::new(Arc::new(Process)));

/// Replaces the environment the gateway's AWS identity is read from; for tests.
#[doc(hidden)]
pub fn use_environment(env: Arc<dyn Environment>) {
    if let Ok(mut e) = ENVIRONMENT.write() {
        *e = env;
    }
}

fn env() -> Arc<dyn Environment> {
    ENVIRONMENT
        .read()
        .map(|e| Arc::clone(&e))
        .unwrap_or_else(|_| Arc::new(Process))
}

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_default()
});

async fn role_credentials(
    role: &Role,
    region: &str,
    now: OffsetDateTime,
) -> Result<Expiring, String> {
    let env = env();
    let (base, expires) = ambient(env.as_ref(), region).await?;
    match &role.role_arn {
        None => Ok((base, expires)),
        Some(arn) => assume_role(
            env.as_ref(),
            &base,
            arn,
            role.external_id.as_deref(),
            region,
            now,
        )
        .await
        .map(|(c, e)| (c, Some(e))),
    }
}

// The gateway's own credentials, in the order AWS's SDKs look for them.
async fn ambient(env: &dyn Environment, region: &str) -> Result<Expiring, String> {
    if let (Some(id), Some(secret)) = (
        env.var("AWS_ACCESS_KEY_ID"),
        env.var("AWS_SECRET_ACCESS_KEY"),
    ) {
        return Ok((
            AwsCredentials {
                access_key_id: id,
                secret_access_key: SecretString::from(secret),
                session_token: env.var("AWS_SESSION_TOKEN").map(SecretString::from),
            },
            None,
        ));
    }
    if let (Some(file), Some(arn)) = (
        env.var("AWS_WEB_IDENTITY_TOKEN_FILE"),
        env.var("AWS_ROLE_ARN"),
    ) {
        let token = tokio::fs::read_to_string(&file)
            .await
            .map_err(|_| "the web identity token file can't be read".to_owned())?;
        return web_identity(env, &arn, token.trim(), region)
            .await
            .map(|(c, e)| (c, Some(e)));
    }
    if let Some(url) = container_url(env) {
        return container(env, &url).await;
    }
    imds().await
}

fn container_url(env: &dyn Environment) -> Option<String> {
    env.var("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI")
        .map(|rel| format!("{ECS_HOST}{rel}"))
        .or_else(|| env.var("AWS_CONTAINER_CREDENTIALS_FULL_URI"))
}

/// The JSON the container endpoint and the instance metadata answer with.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Issued {
    access_key_id: String,
    secret_access_key: String,
    #[serde(default)]
    token: Option<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    expiration: Option<OffsetDateTime>,
}

impl Issued {
    fn into_credentials(self) -> Expiring {
        (
            AwsCredentials {
                access_key_id: self.access_key_id,
                secret_access_key: SecretString::from(self.secret_access_key),
                session_token: self.token.map(SecretString::from),
            },
            self.expiration,
        )
    }
}

// https://docs.aws.amazon.com/sdkref/latest/guide/feature-container-credentials.html
async fn container(env: &dyn Environment, url: &str) -> Result<Expiring, String> {
    let token = match env.var("AWS_CONTAINER_AUTHORIZATION_TOKEN_FILE") {
        Some(file) => Some(
            tokio::fs::read_to_string(file)
                .await
                .map_err(|_| "the container authorization token file can't be read".to_owned())?
                .trim()
                .to_owned(),
        ),
        None => env.var("AWS_CONTAINER_AUTHORIZATION_TOKEN"),
    };
    let mut req = CLIENT.get(url);
    if let Some(t) = token {
        req = req.header("authorization", t);
    }
    let resp = req
        .send()
        .await
        .map_err(|_| "the container credentials endpoint can't be reached".to_owned())?;
    if !resp.status().is_success() {
        return Err(format!(
            "the container credentials endpoint answered {}",
            resp.status().as_u16()
        ));
    }
    resp.text()
        .await
        .ok()
        .and_then(|t| serde_json::from_str::<Issued>(&t).ok())
        .map(Issued::into_credentials)
        .ok_or_else(|| "the container credentials endpoint answered without credentials".to_owned())
}

// https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/instance-metadata-security-credentials.html
async fn imds() -> Result<Expiring, String> {
    let unreachable = |_| "no AWS credentials were found for the gateway".to_owned();
    let token = CLIENT
        .put(format!("{IMDS}/latest/api/token"))
        .header("x-aws-ec2-metadata-token-ttl-seconds", "21600")
        .send()
        .await
        .map_err(unreachable)?
        .text()
        .await
        .map_err(unreachable)?;
    let base = format!("{IMDS}/latest/meta-data/iam/security-credentials/");
    let role = CLIENT
        .get(&base)
        .header("x-aws-ec2-metadata-token", &token)
        .send()
        .await
        .map_err(unreachable)?
        .text()
        .await
        .map_err(unreachable)?;
    let role = role.lines().next().unwrap_or_default().trim();
    if role.is_empty() {
        return Err("the instance has no IAM role".to_owned());
    }
    CLIENT
        .get(format!("{base}{role}"))
        .header("x-aws-ec2-metadata-token", &token)
        .send()
        .await
        .map_err(unreachable)?
        .text()
        .await
        .ok()
        .and_then(|t| serde_json::from_str::<Issued>(&t).ok())
        .map(Issued::into_credentials)
        .ok_or_else(|| "the instance metadata answered without credentials".to_owned())
}

// STS's regional endpoint, or the one `AWS_ENDPOINT_URL_STS` names, such as a VPC endpoint
// (https://docs.aws.amazon.com/sdkref/latest/guide/feature-ss-endpoints.html).
fn sts_url(env: &dyn Environment, region: &str) -> String {
    env.var("AWS_ENDPOINT_URL_STS")
        .unwrap_or_else(|| format!("https://sts.{region}.amazonaws.com"))
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| {
            format!(
                "{k}={}",
                url::form_urlencoded::byte_serialize(v.as_bytes()).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

// https://docs.aws.amazon.com/STS/latest/APIReference/API_AssumeRoleWithWebIdentity.html: a call
// STS takes without a signature, with the token as the proof.
async fn web_identity(
    env: &dyn Environment,
    arn: &str,
    token: &str,
    region: &str,
) -> Result<(AwsCredentials, OffsetDateTime), String> {
    let session = env
        .var("AWS_ROLE_SESSION_NAME")
        .unwrap_or_else(|| SESSION_NAME.to_owned());
    let body = form(&[
        ("Action", "AssumeRoleWithWebIdentity"),
        ("Version", "2011-06-15"),
        ("RoleArn", arn),
        ("RoleSessionName", &session),
        ("WebIdentityToken", token),
    ]);
    sts(&sts_url(env, region), body, None).await
}

// https://docs.aws.amazon.com/STS/latest/APIReference/API_AssumeRole.html, signed with the
// gateway's own credentials.
async fn assume_role(
    env: &dyn Environment,
    base: &AwsCredentials,
    arn: &str,
    external_id: Option<&str>,
    region: &str,
    now: OffsetDateTime,
) -> Result<(AwsCredentials, OffsetDateTime), String> {
    let mut pairs = vec![
        ("Action", "AssumeRole"),
        ("Version", "2011-06-15"),
        ("RoleArn", arn),
        ("RoleSessionName", SESSION_NAME),
    ];
    if let Some(id) = external_id {
        pairs.push(("ExternalId", id));
    }
    let body = form(&pairs);
    let signer = SigV4 {
        credentials: base.clone(),
        region: region.to_owned(),
        service: "sts".to_owned(),
        at: now,
    };
    sts(&sts_url(env, region), body, Some(&signer)).await
}

async fn sts(
    url: &str,
    body: String,
    signer: Option<&SigV4>,
) -> Result<(AwsCredentials, OffsetDateTime), String> {
    const FORM: &str = "application/x-www-form-urlencoded; charset=utf-8";
    let parsed = url::Url::parse(url).map_err(|_| "the STS address isn't a URL".to_owned())?;
    let mut req = CLIENT.post(parsed.as_str()).header("content-type", FORM);
    if let Some(s) = signer {
        let host = match parsed.port() {
            Some(p) => format!("{}:{p}", parsed.host_str().unwrap_or_default()),
            None => parsed.host_str().unwrap_or_default().to_owned(),
        };
        for (name, value) in s
            .sign(&Signing {
                method: "POST",
                host: &host,
                path: parsed.path(),
                query: parsed.query().unwrap_or_default(),
                headers: &[("content-type", FORM)],
                body: body.as_bytes(),
            })
            .headers
        {
            req = req.header(name, value);
        }
    }
    let resp = req
        .body(body)
        .send()
        .await
        .map_err(|_| "AWS STS can't be reached".to_owned())?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|_| "AWS STS's answer can't be read".to_owned())?;
    if !(200..300).contains(&status) {
        let code = tag(&text, "Code").unwrap_or("an error");
        return Err(format!("AWS STS answered {status} with {code}"));
    }
    let (Some(id), Some(secret), Some(token), Some(expires)) = (
        tag(&text, "AccessKeyId"),
        tag(&text, "SecretAccessKey"),
        tag(&text, "SessionToken"),
        tag(&text, "Expiration").and_then(|e| {
            OffsetDateTime::parse(e, &time::format_description::well_known::Rfc3339).ok()
        }),
    ) else {
        return Err("AWS STS answered without credentials".to_owned());
    };
    Ok((
        AwsCredentials {
            access_key_id: id.to_owned(),
            secret_access_key: SecretString::from(unescape(secret)),
            session_token: Some(SecretString::from(unescape(token))),
        },
        expires,
    ))
}

// The text of the first `<name>` element of STS's XML answer.
fn tag<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{name}>"))? + start;
    Some(xml[start..end].trim())
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_access_key_is_read_from_its_json() {
        let k = AccessKey::parse(
            r#"{"access_key_id":"AKIAIOSFODNN7EXAMPLE","secret_access_key":"wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"}"#,
        )
        .unwrap();
        assert_eq!(k.last4(), "MPLE");
        assert!(k.session_token.is_none());
        assert_eq!(AccessKey::parse("nope").err(), Some(Problem::NotJson));
        assert_eq!(
            AccessKey::parse(r#"{"access_key_id":"AKIAIOSFODNN7EXAMPLE"}"#).err(),
            Some(Problem::Missing("secret_access_key"))
        );
        assert_eq!(
            AccessKey::parse(r#"{"access_key_id":"AKIA-1","secret_access_key":"x"}"#).err(),
            Some(Problem::Malformed("access_key_id"))
        );
        assert_eq!(
            AccessKey::parse(
                r#"{"access_key_id":"ASIAIOSFODNN7EXAMPLE","secret_access_key":"x","session_token":""}"#
            )
            .err(),
            Some(Problem::Malformed("session_token"))
        );
        assert_eq!(
            AccessKey::parse(
                r#"{"access_key_id":"ASIAIOSFODNN7EXAMPLE","secret_access_key":"x y"}"#
            )
            .err(),
            Some(Problem::Malformed("secret_access_key"))
        );
    }

    #[test]
    fn a_role_is_an_arn_with_an_optional_external_id() {
        let r = Role::parse(
            r#"{"role_arn":"arn:aws:iam::123456789012:role/kaeso/bedrock","external_id":"tenant-1"}"#,
        )
        .unwrap();
        assert_eq!(r.label(), "bedrock");
        assert_eq!(Role::parse("").unwrap().label(), "gateway");
        assert_eq!(Role::parse("{}").unwrap().label(), "gateway");
        assert_eq!(
            Role::parse(r#"{"role_arn":"arn:aws:iam::1234:role/x"}"#).err(),
            Some(Problem::Malformed("role_arn"))
        );
        assert_eq!(
            Role::parse(r#"{"role_arn":"arn:aws:iam::123456789012:role/x","external_id":"a b"}"#)
                .err(),
            Some(Problem::Malformed("external_id"))
        );
        assert_eq!(Role::parse("[").err(), Some(Problem::NotJson));
    }

    #[test]
    fn sts_answers_are_read_from_their_xml() {
        let xml = "<AssumeRoleResponse><AssumeRoleResult><Credentials><AccessKeyId>ASIA1</AccessKeyId><SecretAccessKey>s&amp;1</SecretAccessKey></Credentials></AssumeRoleResult></AssumeRoleResponse>";
        assert_eq!(tag(xml, "AccessKeyId"), Some("ASIA1"));
        assert_eq!(unescape(tag(xml, "SecretAccessKey").unwrap()), "s&1");
        assert_eq!(tag(xml, "SessionToken"), None);
        assert_eq!(
            form(&[("RoleArn", "arn:aws:iam::1:role/a b")]),
            "RoleArn=arn%3Aaws%3Aiam%3A%3A1%3Arole%2Fa+b"
        );
    }
}
