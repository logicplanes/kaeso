{{- define "kaeso.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "kaeso.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := default .Chart.Name .Values.nameOverride -}}
{{- if contains $name .Release.Name -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "kaeso.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" }}
app.kubernetes.io/name: {{ include "kaeso.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: kaeso
{{- end -}}

{{/* The labels that select one component's pods: gateway, console, postgres and so on. */}}
{{- define "kaeso.selector" -}}
app.kubernetes.io/name: {{ include "kaeso.name" .root }}
app.kubernetes.io/instance: {{ .root.Release.Name }}
app.kubernetes.io/component: {{ .component }}
{{- end -}}

{{- define "kaeso.image" -}}
{{- $image := .image -}}
{{- $ref := printf "%s/%s" .root.Values.image.registry $image.repository -}}
{{- if $image.digest -}}
{{- printf "%s@%s" $ref $image.digest -}}
{{- else -}}
{{- printf "%s:%s" $ref (default .root.Chart.AppVersion $image.tag) -}}
{{- end -}}
{{- end -}}

{{- define "kaeso.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "kaeso.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end -}}

{{- define "kaeso.host" -}}
{{- if .Values.ingress.host -}}
{{- .Values.ingress.host -}}
{{- else -}}
{{- (urlParse (required "set publicUrl, such as https://kaeso.example.com" .Values.publicUrl)).hostname -}}
{{- end -}}
{{- end -}}

{{/* The Secret that holds the chart's own credentials: master keys and the bundled stores'. */}}
{{- define "kaeso.generatedSecret" -}}
{{- printf "%s-generated" (include "kaeso.fullname" .) -}}
{{- end -}}

{{/*
A value of the generated Secret: the one it already holds, so upgrades keep it, or a new one.
*/}}
{{- define "kaeso.generated" -}}
{{- $existing := lookup "v1" "Secret" .root.Release.Namespace (include "kaeso.generatedSecret" .root) -}}
{{- if and $existing $existing.data (hasKey $existing.data .key) -}}
{{- index $existing.data .key | b64dec -}}
{{- else -}}
{{- .new -}}
{{- end -}}
{{- end -}}

{{/* Where each connection URL comes from: the bundled stores' Secret, or the operator's. */}}
{{- define "kaeso.storeUrl" -}}
{{- $root := .root -}}
{{- if $root.Values.bundled.enabled -}}
name: {{ include "kaeso.generatedSecret" $root }}
key: {{ .store }}-url
{{- else -}}
{{- $store := index $root.Values .store -}}
name: {{ required (printf "set %s.existingSecret to the Secret with its URL, or bundled.enabled to try Kaeso out" .store) $store.existingSecret }}
key: {{ $store.key }}
{{- end -}}
{{- end -}}

{{- define "kaeso.masterKeysSecret" -}}
{{- default (include "kaeso.generatedSecret" .) .Values.masterKeys.existingSecret -}}
{{- end -}}

{{- define "kaeso.masterKeysKey" -}}
{{- if .Values.masterKeys.existingSecret -}}{{ .Values.masterKeys.key }}{{- else -}}master-keys{{- end -}}
{{- end -}}

{{/* The gateway's environment, which the migration Job shares so both read the same settings. */}}
{{- define "kaeso.gatewayEnv" -}}
- name: KAESO_PUBLIC_URL
  value: {{ required "set publicUrl, such as https://kaeso.example.com" .Values.publicUrl | quote }}
- name: KAESO_UPDATES_INSTALLED_WITH
  value: helm
- name: KAESO_LOG_FORMAT
  value: json
- name: KAESO_TRUSTED_PROXIES
  value: {{ .Values.gateway.trustedProxies | quote }}
{{- $extra := list }}
{{- range .Values.gateway.extraEnv }}{{ $extra = append $extra .name }}{{ end }}
{{- if or (hasKey .Values.gateway.env "KAESO_SEARCH_ENGINES") (hasKey .Values.gateway.config "search_engines") (has "KAESO_SEARCH_ENGINES" $extra) }}
{{- fail "set searchEngines instead: the console reads it too" }}
{{- end }}
- name: KAESO_SEARCH_ENGINES
  value: {{ .Values.searchEngines | quote }}
{{- if .Values.gateway.metrics.enabled }}
- name: KAESO_METRICS_LISTEN
  value: {{ printf "0.0.0.0:%v" .Values.gateway.metrics.port | quote }}
{{- end }}
- name: KAESO_MASTER_KEYS_FILE
  value: /run/secrets/kaeso/master-keys
- name: KAESO_POSTGRES_URL
  valueFrom:
    secretKeyRef:
      {{- include "kaeso.storeUrl" (dict "root" . "store" "postgres") | nindent 6 }}
{{- if or .Values.bundled.enabled .Values.clickhouse.existingSecret }}
- name: KAESO_CLICKHOUSE_URL
  valueFrom:
    secretKeyRef:
      {{- include "kaeso.storeUrl" (dict "root" . "store" "clickhouse") | nindent 6 }}
{{- end }}
- name: KAESO_VALKEY_URL
  valueFrom:
    secretKeyRef:
      {{- include "kaeso.storeUrl" (dict "root" . "store" "valkey") | nindent 6 }}
{{- if .Values.valkey.ca }}
- name: KAESO_VALKEY_CA
  value: {{ .Values.valkey.ca | quote }}
{{- end }}
- name: KAESO_MAIL_TRANSPORT
  value: {{ .Values.mail.transport | quote }}
{{- with .Values.mail.smtp }}
{{- if .host }}
- name: KAESO_SMTP_HOST
  value: {{ .host | quote }}
{{- end }}
- name: KAESO_SMTP_PORT
  value: {{ .port | quote }}
- name: KAESO_SMTP_TLS
  value: {{ .tls | quote }}
{{- if .user }}
- name: KAESO_SMTP_USER
  value: {{ .user | quote }}
{{- end }}
{{- if .from }}
- name: KAESO_SMTP_FROM
  value: {{ .from | quote }}
{{- end }}
{{- if .password.existingSecret }}
- name: KAESO_SMTP_PASSWORD
  valueFrom:
    secretKeyRef:
      name: {{ .password.existingSecret }}
      key: {{ .password.key }}
{{- end }}
{{- end }}
{{- if .Values.mail.setupEmail }}
- name: KAESO_SETUP_EMAIL
  value: {{ .Values.mail.setupEmail | quote }}
{{- end }}
{{- if .Values.gateway.config }}
- name: KAESO_CONFIG
  value: /etc/kaeso/kaeso.toml
{{- end }}
{{- range $name, $value := .Values.gateway.env }}
- name: {{ $name }}
  value: {{ $value | toString | quote }}
{{- end }}
{{- with .Values.gateway.extraEnv }}
{{ toYaml . }}
{{- end }}
{{- end -}}

{{/* The gateway's volumes and mounts: the master keys as a file, and the configuration file. */}}
{{- define "kaeso.gatewayVolumes" -}}
- name: master-keys
  secret:
    secretName: {{ include "kaeso.masterKeysSecret" .root }}
    defaultMode: 0440
    items:
      - key: {{ include "kaeso.masterKeysKey" .root }}
        path: master-keys
{{- if .root.Values.gateway.config }}
- name: config
  configMap:
    name: {{ .configMap }}
{{- end }}
{{- end -}}

{{- define "kaeso.gatewayMounts" -}}
- name: master-keys
  mountPath: /run/secrets/kaeso
  readOnly: true
{{- if .Values.gateway.config }}
- name: config
  mountPath: /etc/kaeso
  readOnly: true
{{- end }}
{{- end -}}

{{/*
A TOML document from a map of settings: tables for maps, quoted strings, bare numbers and
booleans, and arrays of plain values.
*/}}
{{- define "kaeso.toml" -}}
{{- $tables := dict -}}
{{- range $key, $value := .values -}}
{{- if kindIs "map" $value -}}
{{- $_ := set $tables $key $value -}}
{{- else }}
{{ $key }} = {{ include "kaeso.tomlValue" $value }}
{{- end -}}
{{- end -}}
{{- range $key, $value := $tables }}
[{{ if $.prefix }}{{ $.prefix }}.{{ end }}{{ $key }}]
{{- include "kaeso.toml" (dict "values" $value "prefix" (ternary (printf "%s.%s" $.prefix $key) $key (ne $.prefix ""))) }}
{{- end -}}
{{- end -}}

{{- define "kaeso.tomlValue" -}}
{{- if kindIs "string" . -}}
{{- toJson . -}}
{{- else if kindIs "slice" . -}}
[{{ range $i, $v := . }}{{ if $i }}, {{ end }}{{ include "kaeso.tomlValue" $v }}{{ end }}]
{{- else if kindIs "float64" . -}}
{{- if eq (floor .) . }}{{ int64 . }}{{ else }}{{ . }}{{ end -}}
{{- else -}}
{{- . -}}
{{- end -}}
{{- end -}}

{{/*
The paths the gateway answers, as prefixes: the model APIs, the Platform API, the MCP portal, the
sign-in of agents and what sources report. Everything else is the console's.
*/}}
{{- define "kaeso.gatewayPaths" -}}
{{- list "/v1" "/v1beta" "/v2" "/platform" "/mcp" "/oauth" "/ingest" "/.well-known/oauth-authorization-server" "/.well-known/oauth-protected-resource" | toJson -}}
{{- end -}}

{{/* A bundled store's StatefulSet volume claim. */}}
{{- define "kaeso.bundledClaim" -}}
volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: [ReadWriteOnce]
      {{- with .root.Values.bundled.storageClass }}
      storageClassName: {{ . }}
      {{- end }}
      resources:
        requests:
          storage: {{ .size }}
{{- end -}}

{{- define "kaeso.generatedRef" -}}
secretKeyRef:
  name: {{ include "kaeso.generatedSecret" .root }}
  key: {{ .key }}
{{- end -}}
