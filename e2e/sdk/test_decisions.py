import httpx
import openai
import pytest
from google import genai
from google.genai import types
from google.genai._interactions import APIStatusError
from typesafe_sdk import RetryPolicy, TypeSafeAPIError, TypeSafeClient

QUESTIONS = {
    "refund": {"type": "noul", "instructions": "Does the customer ask for a refund?"},
    "team": {"type": "choice", "instructions": "Which team handles it?", "criteria": {"billing": "Payments.", "support": "Everything else."}},
    "urgency": {"type": "score", "instructions": "How urgent is it?", "criteria": ["low", "medium", "high"]},
}
STATE = "I was charged twice for my order; please refund me."


@pytest.fixture
def ts(install) -> TypeSafeClient:
    with TypeSafeClient(api_key=install.key, base_url=install.url, retry=RetryPolicy(max_retries=0)) as c:
        yield c


@pytest.fixture
def gd(install) -> genai.Client:
    return genai.Client(
        api_key=install.key,
        http_options=types.HttpOptions(base_url=install.url, api_version="v1beta", retry_options=types.HttpRetryOptions(attempts=1)),
    )


@pytest.mark.parametrize(
    ("model", "path"),
    [("gpt-6-luna", "/v1/decisions"), ("gemini-decision-1", "/v1beta/decisions"), ("typesafe/jev-1.14", "/v1/systemone")],
)
def test_openai_sdk_decides_on_every_decisions_provider(oa: openai.OpenAI, mock, model, path):
    got = oa.decisions.create(
        model=model,
        input=STATE,
        questions=[
            {"type": "predicate", "name": "refund", "instructions": "Does the customer ask for a refund?"},
            {"type": "choice", "name": "team", "instructions": "Which team?", "choices": [{"value": "billing"}, {"value": "support"}]},
            {"type": "score", "name": "urgency", "instructions": "How urgent?", "levels": [{"label": "low"}, {"label": "high"}]},
        ],
    )
    assert [a.type for a in got.answers] == ["predicate", "choice", "score"]
    assert got.answers[0].probability == pytest.approx(0.82)
    assert got.answers[1].choice == "billing"
    assert got.usage.input_tokens > 0
    assert mock.last()["path"] == path


@pytest.mark.parametrize("model", ["gpt-6-luna", "gemini-decision-1", "typesafe/jev-1.14"])
def test_typesafe_sdk_decides_through_system_one(ts: TypeSafeClient, model):
    got = ts.system_one(STATE, QUESTIONS, model=model)
    assert list(got.answers) == list(QUESTIONS)
    assert got.answers["refund"].noul == pytest.approx(0.82)
    assert got.answers["team"].choice == "billing"
    assert len(got.answers["urgency"].legend) == 3
    assert got.usage.input_tokens > 0


def test_typesafe_sdk_sends_compressed_bodies_kaeso_reads(install, mock):
    with TypeSafeClient(api_key=install.key, base_url=install.url, retry=RetryPolicy(max_retries=0)) as c:
        got = c.system_one(STATE * 40, QUESTIONS, model="typesafe/jev-1.14")
    assert got.answers["refund"].noul == pytest.approx(0.82)
    assert mock.last()["path"] == "/v1/systemone"


@pytest.mark.parametrize("model", ["gemini-decision-1", "gpt-6-luna", "typesafe/jev-1.14"])
def test_google_genai_decides_through_gemini_decisions(gd: genai.Client, model):
    got = gd.decisions.create(
        model=model,
        input=STATE,
        questions=QUESTIONS,
    )
    assert list(got.answers) == list(QUESTIONS)
    assert got.answers["refund"].noul == pytest.approx(0.82)
    assert got.usage.total_input_tokens > 0


def test_each_sdk_reads_kaesos_refusals_in_its_own_format(oa, ts, gd):
    with pytest.raises(openai.BadRequestError) as refused:
        oa.decisions.create(model="gpt-4o", input=STATE, questions=[{"type": "predicate", "instructions": "A refund?"}])
    assert "generation model" in refused.value.message
    with pytest.raises(TypeSafeAPIError) as refused:
        ts.system_one(STATE, {"where": {"type": "box", "instructions": "the receipt"}}, model="gpt-6-luna")
    assert refused.value.status == 400
    assert "box questions" in str(refused.value.body["detail"])
    with pytest.raises(APIStatusError) as refused:
        gd.decisions.create(model="claude-haiku-4-5", input=STATE, questions={"q": {"type": "noul", "instructions": "A refund?"}})
    assert refused.value.status_code == 400
    assert "decisions API" in refused.value.response.json()["error"]["message"]


def test_a_system_one_body_at_the_openai_path_is_pointed_to_systemone(install):
    r = httpx.post(
        f"{install.url}/v1/decisions",
        json={"model": "gpt-6-luna", "state": STATE, "questions": QUESTIONS},
        headers={"authorization": f"Bearer {install.key}"},
    )
    assert r.status_code == 400
    assert "/v1/systemone" in r.json()["error"]["message"]
