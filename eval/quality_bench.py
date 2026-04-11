#!/usr/bin/env python3
"""
quality_bench.py — LLM 답변 품질 비교 벤치마크

Raw MD  vs  Transpiled (Semantic)  vs  Transpiled (Compressed)
동일 문서 + 동일 질문 → Gemini Flash 2.0 / Claude 3.5 Haiku 답변 품질 측정

사용법:
  python3 eval/quality_bench.py
"""

import os, subprocess, json, time, urllib.request, urllib.error
from pathlib import Path

ROOT     = Path(__file__).parent.parent
DATASET  = ROOT / "eval/dataset/hf"
TRANSPILE = ROOT / "target/debug/transpile"

OPENROUTER_KEY  = os.environ["OPENROUTER_API_KEY"]
ANSWER_MODEL    = "google/gemini-2.0-flash-001"      # 답변 생성
JUDGE_MODEL     = "anthropic/claude-haiku-4-5"       # 품질 판정 (다른 모델로 교차 검증)

# ── 테스트 케이스 ────────────────────────────────────────────────────────
TESTS = [
    {
        "file": "hub-docs_security.md",
        "questions": [
            "What are the two types of User Access Tokens described?",
            "What should you do immediately if a token is leaked or compromised?",
            "Is Hugging Face SOC2 certified, and what type?",
        ],
    },
    {
        "file": "repositories-getting-started.md",
        "questions": [
            "What version control system does Hugging Face use for repositories?",
            "What command-line tool is recommended for cloning large repositories?",
            "What is the maximum file size recommended without Git LFS?",
        ],
    },
    {
        "file": "hub-docs_spaces_docker.md",
        "questions": [
            "What port must a Docker Space expose by default?",
            "What is the maximum disk size for a free-tier Docker Space?",
            "What environment variable contains the Space's subdomain URL?",
        ],
    },
    {
        "file": "hub-docs_api.md",
        "questions": [
            "What Python client library is provided for Hub API access?",
            "Where has the Hub API Endpoints documentation been relocated?",
            "In what format can you retrieve the API spec for sending to an AI agent?",
        ],
    },
    {
        "file": "hub-docs_model_cards_metadata.md",
        "questions": [
            "What file in a repository is used as the model card?",
            "Which metadata field specifies the ML task type?",
            "What section of the model card must contain the YAML front matter?",
        ],
    },
]

# ── OpenRouter 호출 ───────────────────────────────────────────────────────

def chat(model: str, system: str, user: str, max_tokens: int = 300) -> tuple[str, float]:
    payload = json.dumps({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user",   "content": user},
        ],
        "max_tokens": max_tokens,
        "temperature": 0,
    }).encode()
    req = urllib.request.Request(
        "https://openrouter.ai/api/v1/chat/completions",
        data=payload,
        headers={
            "Authorization": f"Bearer {OPENROUTER_KEY}",
            "Content-Type": "application/json",
        },
    )
    t0 = time.time()
    with urllib.request.urlopen(req, timeout=60) as resp:
        d = json.loads(resp.read())
    elapsed = time.time() - t0
    return d["choices"][0]["message"]["content"].strip(), elapsed


def ask(context: str, question: str) -> tuple[str, float]:
    system = (
        "You are a precise document QA assistant. "
        "Answer using ONLY information from the provided document. "
        "Be concise: 1-3 sentences max."
    )
    user = f"=== DOCUMENT ===\n{context}\n=== END ===\n\nQuestion: {question}"
    return chat(ANSWER_MODEL, system, user, max_tokens=200)


def judge(question: str, raw_ans: str, sem_ans: str, cmp_ans: str) -> dict:
    system = (
        "You are a strict QA evaluator. "
        "Score each answer 0-10 (accuracy + completeness + relevance). "
        "Reply ONLY with valid JSON, absolutely no other text."
    )
    user = (
        f"Question: {question}\n\n"
        f"Answer_raw: {raw_ans}\n\n"
        f"Answer_semantic: {sem_ans}\n\n"
        f"Answer_compressed: {cmp_ans}\n\n"
        'JSON format: {"raw":<int>,"semantic":<int>,"compressed":<int>,'
        '"best":"<raw|semantic|compressed>","note":"<one sentence>"}'
    )
    out, _ = chat(JUDGE_MODEL, system, user, max_tokens=150)
    s = out.find("{"); e = out.rfind("}") + 1
    if s >= 0 and e > s:
        try:
            return json.loads(out[s:e])
        except json.JSONDecodeError:
            pass
    return {"raw": 5, "semantic": 5, "compressed": 5, "best": "tie", "note": out[:100]}


# ── 트랜스파일 ────────────────────────────────────────────────────────────

def transpile(text: str, fidelity: str, budget: int) -> tuple[str, int, int]:
    r = subprocess.run(
        [str(TRANSPILE), "-f", "markdown", "-l", fidelity, "-b", str(budget), "-j"],
        input=text, capture_output=True, text=True,
    )
    if r.returncode != 0:
        return text, 0, 0
    d = json.loads(r.stdout)
    return d["content"], d["input_tok"], d["output_tok"]


# ── 메인 ─────────────────────────────────────────────────────────────────

def run():
    print("=" * 74)
    print("  LLM 답변 품질 벤치마크")
    print(f"  답변 모델: {ANSWER_MODEL}")
    print(f"  판정 모델: {JUDGE_MODEL} (교차 검증)")
    print("=" * 74)

    all_scores   = {"raw": [], "semantic": [], "compressed": []}
    wins         = {"raw": 0, "semantic": 0, "compressed": 0, "tie": 0}
    tok_savings  = []
    results_full = []

    for test in TESTS:
        fpath   = DATASET / test["file"]
        raw     = fpath.read_text()
        name    = test["file"]

        sem_text, in_tok, sem_tok = transpile(raw, "semantic",   4096)
        cmp_text, _,      cmp_tok = transpile(raw, "compressed", 2048)

        sem_pct = round((1 - sem_tok / in_tok) * 100, 1) if in_tok else 0
        cmp_pct = round((1 - cmp_tok / in_tok) * 100, 1) if in_tok else 0
        tok_savings.append(sem_pct)

        print(f"\n📄 {name}")
        print(f"   {in_tok} tok  →  Semantic {sem_tok} ({sem_pct:+.1f}%)  |  Compressed {cmp_tok} ({cmp_pct:+.1f}%)")
        print()

        doc_qs = []
        for q in test["questions"]:
            print(f"  Q: {q}")

            raw_ans, rt = ask(raw,      q)
            sem_ans, st = ask(sem_text, q)
            cmp_ans, ct = ask(cmp_text, q)

            scores = judge(q, raw_ans, sem_ans, cmp_ans)
            best   = scores.get("best", "tie")

            all_scores["raw"].append(scores.get("raw", 5))
            all_scores["semantic"].append(scores.get("semantic", 5))
            all_scores["compressed"].append(scores.get("compressed", 5))
            wins[best] = wins.get(best, 0) + 1

            r_s = scores.get("raw",        "?")
            s_s = scores.get("semantic",   "?")
            c_s = scores.get("compressed", "?")
            print(f"  점수    Raw={r_s}  Semantic={s_s}  Compressed={c_s}  →  최고: {best.upper()}")
            print(f"  판정    {scores.get('note','')}")
            print(f"  시간    Raw {rt:.1f}s | Sem {st:.1f}s | Cmp {ct:.1f}s")
            print(f"  Raw:  {raw_ans[:110]!r}")
            print(f"  Sem:  {sem_ans[:110]!r}")
            print(f"  Cmp:  {cmp_ans[:110]!r}")
            print()

            doc_qs.append({"q": q, "scores": scores,
                           "raw": raw_ans, "sem": sem_ans, "cmp": cmp_ans})

        results_full.append({"file": name, "questions": doc_qs,
                             "in_tok": in_tok, "sem_tok": sem_tok, "cmp_tok": cmp_tok})

    # ── 최종 요약 ─────────────────────────────────────────────────────────
    avg        = {k: sum(v)/len(v) for k, v in all_scores.items() if v}
    avg_saving = sum(tok_savings) / len(tok_savings)
    sem_drop   = avg["raw"] - avg["semantic"]
    cmp_drop   = avg["raw"] - avg["compressed"]
    n          = len(all_scores["raw"])

    print()
    print("=" * 74)
    print("  최종 결과 요약")
    print("=" * 74)
    print(f"  {'':18} {'Raw MD':>8}  {'Semantic':>10}  {'Compressed':>12}")
    print(f"  평균 점수 (/10)    {avg['raw']:>8.2f}  {avg['semantic']:>10.2f}  {avg['compressed']:>12.2f}")
    print(f"  1등 횟수 (/{n})    {wins.get('raw',0):>8}  {wins.get('semantic',0):>10}  {wins.get('compressed',0):>12}")
    print(f"  평균 토큰 절감     {'기준':>8}  {avg_saving:>9.1f}%  {'더 절감':>12}")
    print()
    print(f"  품질 델타 (vs Raw MD)")
    print(f"    Semantic:   {sem_drop:+.2f}점  |  토큰 {avg_saving:.1f}% 절감")
    print(f"    Compressed: {cmp_drop:+.2f}점  |  토큰 더 절감")
    print()

    # 판정
    if abs(sem_drop) < 0.3:
        verdict = "✅  Semantic: 품질 손실 없음 — 실무 투입 권장"
    elif sem_drop < 1.0:
        verdict = "⚠️   Semantic: 미미한 품질 저하 — 비용 우선 시 고려 가능"
    elif sem_drop < 2.0:
        verdict = "🔶  Semantic: 유의미한 품질 저하 — 재검토 필요"
    else:
        verdict = "❌  Semantic: 품질 저하 과도 — 실무 투입 부적합"
    print(f"  최종 판정: {verdict}")

    out = ROOT / "eval/quality_bench_results.json"
    out.write_text(json.dumps({
        "models":    {"answer": ANSWER_MODEL, "judge": JUDGE_MODEL},
        "summary":   {"avg": avg, "wins": wins,
                      "avg_token_saving_pct": avg_saving,
                      "sem_quality_drop": sem_drop,
                      "cmp_quality_drop": cmp_drop},
        "documents": results_full,
    }, ensure_ascii=False, indent=2))
    print(f"\n  상세 결과 저장: {out}")


if __name__ == "__main__":
    run()
