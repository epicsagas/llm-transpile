//! stats_report — HTML dashboard for transpile usage stats

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::BufRead;
use std::path::PathBuf;

// ── Data model ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatsRecord {
    pub ts: String,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub project: String,
    pub file: String,
    #[allow(dead_code)]
    pub format: String,
    pub fidelity: String,
    pub input_tok: usize,
    pub output_tok: usize,
    pub reduction_pct: f64,
    pub saved: usize,
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn cmd_stats_report(
    days: u32,
    agent_filter: Option<String>,
    project_filter: Option<String>,
    out: &str,
    no_open: bool,
) -> i32 {
    let days = days.min(90);
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => {
            eprintln!("error: HOME environment variable not set");
            return 1;
        }
    };
    let stats_dir = PathBuf::from(&home).join(".agents/transpile/stats");

    println!("▶ stats report");
    println!("  dir  : {}", stats_dir.display());
    println!("  days : {days}");

    let records = load_stats(&stats_dir, days);

    let mut filtered = records;
    if let Some(ref a) = agent_filter {
        filtered.retain(|r| r.agent == *a);
    }
    if let Some(ref p) = project_filter {
        filtered.retain(|r| r.project == *p);
    }

    if filtered.is_empty() {
        eprintln!("ERROR: no stats found for the given range. Run transpile on some files first.");
        return 1;
    }

    println!("  records : {}", filtered.len());
    let out = expand_tilde(out);
    if let Some(parent) = std::path::Path::new(&out).parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = fs::create_dir_all(parent)
    {
        eprintln!("ERROR: cannot create directory {}: {e}", parent.display());
        return 1;
    }
    generate_html(&filtered, &out);
    println!("  ✓ {out}");

    if !no_open {
        let _ = std::process::Command::new("open").arg(&out).spawn();
    }

    0
}

// ── Data loading ──────────────────────────────────────────────────────────────

fn load_stats(stats_dir: &std::path::Path, days: u32) -> Vec<StatsRecord> {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let today_days = now_secs / 86400;

    let mut all: Vec<StatsRecord> = Vec::new();

    for offset in 0..days as u64 {
        let day = today_days.saturating_sub(offset);
        let (y, m, d) = epoch_days_to_ymd(day);
        let date_str = format!("{y:04}-{m:02}-{d:02}");
        let path = stats_dir.join(format!("{date_str}.jsonl"));
        if let Ok(f) = fs::File::open(&path) {
            for line in std::io::BufReader::new(f).lines().map_while(|r| r.ok()) {
                if let Ok(rec) = serde_json::from_str::<StatsRecord>(&line) {
                    all.push(rec);
                }
            }
        }
    }
    all
}

fn epoch_days_to_ymd(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn js_safe(json: &str) -> String {
    json.replace("</", "<\\/")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{home}/{rest}");
    }
    path.to_string()
}

// ── HTML generation ───────────────────────────────────────────────────────────

fn generate_html(records: &[StatsRecord], out_path: &str) {
    // Serialize all records as JSON for client-side filtering
    let records_json = js_safe(&serde_json::to_string(records).unwrap());

    // Collect unique filter values
    let mut projects: Vec<String> = records.iter()
        .map(|r| if r.project.is_empty() { "unknown".to_string() } else { r.project.clone() })
        .collect::<std::collections::HashSet<_>>().into_iter().collect();
    projects.sort();
    let mut agents: Vec<String> = records.iter()
        .map(|r| if r.agent.is_empty() { "unknown".to_string() } else { r.agent.clone() })
        .collect::<std::collections::HashSet<_>>().into_iter().collect();
    agents.sort();

    let project_options = projects.iter().map(|p| format!("<option>{}</option>", esc(p))).collect::<Vec<_>>().join("");
    let agent_options = agents.iter().map(|a| format!("<option>{}</option>", esc(a))).collect::<Vec<_>>().join("");

    let html = format!(r##"<!DOCTYPE html>
<html lang="en" data-lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>stats — llm-transpile usage report</title>
<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.2/dist/chart.umd.min.js" integrity="sha384-e6cc9LaIG7xZ3XD5B+jtr1NhTWPQGQdRCh6xiZ+ZFUtWCpg4ycv3Sh+SkZoopvUY" crossorigin="anonymous"></script>
<style>
:root{{--bg:#0f1117;--surf:#1a1d27;--bdr:#2e3147;--txt:#e2e8f0;--mut:#8892a4;
  --acc:#6366f1;--grn:#22c55e;--ylw:#eab308;--red:#ef4444;--thead:#1e2235;}}
:root[data-theme="light"]{{--bg:#f5f5f7;--surf:#ffffff;--bdr:#d1d5db;--txt:#1e293b;--mut:#374151;
  --acc:#4f46e5;--grn:#16a34a;--ylw:#ca8a04;--red:#dc2626;--thead:#f1f5f9;}}
*{{box-sizing:border-box;margin:0;padding:0;}}
body{{background:var(--bg);color:var(--txt);font-family:system-ui,sans-serif;font-size:14px;}}
header{{padding:16px 28px;border-bottom:1px solid var(--bdr);display:flex;align-items:center;gap:12px;flex-wrap:wrap;}}
header h1{{font-size:20px;font-weight:700;}}
.badge{{background:var(--acc);color:#fff;font-size:11px;padding:2px 8px;border-radius:99px;font-weight:600;}}
.lang-btn{{background:transparent;border:1px solid var(--bdr);color:var(--mut);
  border-radius:7px;padding:4px 12px;font-size:12px;cursor:pointer;font-weight:600;transition:all .15s;}}
.lang-btn:hover{{border-color:var(--acc);color:var(--txt);}}
.hdr-actions{{display:flex;gap:6px;flex-shrink:0;}}
.hdr-meta{{font-size:12px;color:var(--mut);}}
.filters{{background:var(--surf);border-bottom:1px solid var(--bdr);padding:10px 28px;
  display:flex;gap:10px;flex-wrap:wrap;align-items:center;position:sticky;top:0;z-index:50;}}
.filters label{{font-size:11px;color:var(--mut);text-transform:uppercase;letter-spacing:.3px;font-weight:600;}}
.preset{{background:var(--surf);border:1px solid var(--bdr);color:var(--mut);
  border-radius:7px;padding:4px 10px;font-size:11px;cursor:pointer;font-weight:600;transition:all .15s;}}
.preset:hover{{border-color:var(--acc);color:var(--txt);}}
.preset.active{{background:var(--acc);border-color:var(--acc);color:#fff;}}
.sep{{width:1px;height:20px;background:var(--bdr);flex-shrink:0;}}
/* ── Date range picker ── */
.dr{{position:relative;display:inline-block;}}
.dr-input{{background:var(--surf);border:1px solid var(--bdr);border-radius:7px;
  padding:4px 10px 4px 28px;font-size:12px;color:var(--txt);cursor:pointer;font-family:inherit;
  white-space:nowrap;position:relative;}}
.dr-input::before{{content:"📅";position:absolute;left:8px;top:50%;transform:translateY(-50%);font-size:13px;}}
.dr-input:hover{{border-color:var(--acc);}}
.dr-input.picking{{border-color:var(--acc);box-shadow:0 0 0 2px rgba(99,102,241,.25);}}
.dr-cal{{display:none;position:absolute;top:calc(100% + 4px);left:0;z-index:99;
  background:var(--surf);border:1px solid var(--bdr);border-radius:10px;padding:14px;
  box-shadow:0 8px 24px rgba(0,0,0,.25);user-select:none;min-width:280px;}}
.dr-cal.open{{display:block;}}
.dr-hdr{{display:flex;align-items:center;justify-content:space-between;margin-bottom:10px;}}
.dr-hdr span{{font-size:13px;font-weight:600;min-width:120px;text-align:center;}}
.dr-hdr button{{background:transparent;border:1px solid var(--bdr);color:var(--mut);
  border-radius:5px;width:26px;height:26px;cursor:pointer;font-size:14px;display:flex;align-items:center;justify-content:center;}}
.dr-hdr button:hover{{border-color:var(--acc);color:var(--txt);}}
.dr-hint{{font-size:10px;color:var(--acc);text-align:center;margin-bottom:8px;min-height:14px;}}
.dr-grid{{display:grid;grid-template-columns:repeat(7,1fr);gap:2px;text-align:center;}}
.dr-grid .dh{{font-size:10px;color:var(--mut);font-weight:600;padding:4px 0;}}
.dr-grid .dc{{font-size:12px;padding:5px 0;border-radius:5px;cursor:pointer;transition:background .1s;}}
.dr-grid .dc:hover{{background:rgba(99,102,241,.15);}}
.dr-grid .dc.today{{font-weight:700;}}
.dr-grid .dc.sel-start{{background:var(--acc);color:#fff;border-radius:5px 0 0 5px;}}
.dr-grid .dc.sel-end{{background:var(--acc);color:#fff;border-radius:0 5px 5px 0;}}
.dr-grid .dc.sel-start.sel-end{{border-radius:5px;}}
.dr-grid .dc.range{{background:rgba(99,102,241,.12);border-radius:0;}}
.dr-grid .dc.preview{{background:rgba(99,102,241,.08);border-radius:0;}}
.dr-grid .dc.out{{color:var(--mut);opacity:.4;cursor:default;}}
.dr-grid .dc.out:hover{{background:transparent;}}
.dr-footer{{display:flex;justify-content:flex-end;gap:6px;margin-top:10px;border-top:1px solid var(--bdr);padding-top:10px;}}
.dr-footer button{{font-size:11px;padding:4px 10px;}}
.dr-footer .btn-clear{{background:transparent;border:1px solid var(--bdr);color:var(--mut);}}
.dr-footer .btn-clear:hover{{border-color:var(--red);color:var(--red);opacity:1;}}
.wrap{{max-width:1400px;margin:0 auto;padding:20px 28px;}}
.cards{{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:14px;margin-bottom:28px;}}
.card{{background:var(--surf);border:1px solid var(--bdr);border-radius:10px;padding:18px;cursor:help;}}
.card .lbl{{font-size:11px;color:var(--mut);text-transform:uppercase;letter-spacing:.5px;margin-bottom:6px;}}
.card .val{{font-size:26px;font-weight:700;}}
.card .sub{{font-size:11px;color:var(--mut);margin-top:3px;}}
.charts{{display:grid;grid-template-columns:1fr 1fr;gap:18px;margin-bottom:28px;}}
@media(max-width:1200px){{.charts{{grid-template-columns:1fr 1fr;gap:14px;}}}}
@media(max-width:768px){{
  .charts{{grid-template-columns:1fr;gap:12px;}}
  .cards{{grid-template-columns:repeat(auto-fit,minmax(130px,1fr));gap:10px;}}
  header{{padding:12px 16px;flex-wrap:wrap;}}
  header h1{{font-size:16px;}}
  .hdr-meta{{display:none;}}
  .filters{{padding:8px 16px;gap:6px;}}
  .wrap{{padding:14px 16px;}}
  .cbox{{padding:14px;}}
  .cbox canvas{{max-height:200px;}}
  table{{font-size:12px;}}
  th,td{{padding:5px 6px;}}
}}
@media(max-width:480px){{
  .cards{{grid-template-columns:1fr 1fr;}}
  .card .val{{font-size:22px;}}
  .filters{{flex-direction:column;align-items:stretch;gap:6px;}}
  .filters label{{display:none;}}
  .sep{{display:none;}}
  header h1{{font-size:14px;}}
  .badge{{font-size:10px;padding:2px 6px;}}
  table{{display:block;overflow-x:auto;}}
}}
.cbox{{background:var(--surf);border:1px solid var(--bdr);border-radius:10px;padding:18px;}}
.cbox h3{{font-size:11px;font-weight:600;color:var(--mut);text-transform:uppercase;letter-spacing:.5px;margin-bottom:14px;}}
.cbox canvas{{max-height:240px;}}
section{{margin-bottom:28px;}}
section h2{{font-size:15px;font-weight:600;margin-bottom:12px;padding-bottom:7px;border-bottom:1px solid var(--bdr);}}
table{{width:100%;border-collapse:collapse;background:var(--surf);border-radius:10px;overflow:hidden;border:1px solid var(--bdr);}}
thead tr{{background:var(--thead);}}
th{{padding:9px 12px;text-align:left;font-size:11px;font-weight:600;color:var(--mut);
  text-transform:uppercase;letter-spacing:.5px;white-space:nowrap;}}
td{{padding:8px 12px;border-top:1px solid var(--bdr);font-size:13px;white-space:nowrap;}}
td.n{{text-align:right;font-variant-numeric:tabular-nums;}}
tr:hover td{{background:rgba(255,255,255,.02);}}
.good{{color:var(--grn);}} .ok{{color:var(--ylw);}} .low{{color:var(--red);}}
input[type=text],input[type=date],select{{background:var(--surf);border:1px solid var(--bdr);border-radius:7px;
  padding:5px 10px;color:var(--txt);font-size:13px;outline:none;}}
input:focus,select:focus{{border-color:var(--acc);}}
button{{background:var(--acc);border:none;color:#fff;border-radius:7px;
  padding:5px 12px;font-size:13px;cursor:pointer;font-weight:600;}}
button:hover{{opacity:.85;}}
.legend{{display:flex;gap:16px;flex-wrap:wrap;margin-bottom:10px;font-size:12px;color:var(--mut);}}
.legend span{{display:flex;align-items:center;gap:5px;}}
.dot{{width:10px;height:10px;border-radius:50%;display:inline-block;}}
</style>
</head>
<body>
<div class="wrap">
<header>
  <h1>stats</h1>
  <span class="badge">llm-transpile</span>
  <span class="hdr-meta" id="hdrMeta"></span>
  <span class="hdr-actions">
    <button class="lang-btn" onclick="toggleLang()" id="langBtn">한국어</button>
    <button class="lang-btn" onclick="toggleTheme()" id="themeBtn">☀</button>
  </span>
</header>

<!-- ── Global filters ── -->
<div class="filters" style="margin-bottom:18px;">
  <button class="preset" data-days="1" onclick="setPreset(this)" data-i18n="preset_today">Today</button>
  <button class="preset active" data-days="7" onclick="setPreset(this)">1W</button>
  <button class="preset" data-days="14" onclick="setPreset(this)">2W</button>
  <button class="preset" data-days="30" onclick="setPreset(this)">1M</button>
  <button class="preset" data-days="90" onclick="setPreset(this)">90D</button>
  <span class="sep"></span>
  <div class="dr">
    <input type="hidden" id="ffrom">
    <input type="hidden" id="fto">
    <div class="dr-input" id="drDisplay" onclick="toggleCal();event.stopPropagation()">Date range</div>
    <div class="dr-cal" id="drCal" onclick="event.stopPropagation()">
      <div class="dr-hdr">
        <button onclick="calNav(-1)">◀</button>
        <span id="calTitle"></span>
        <button onclick="calNav(1)">▶</button>
      </div>
      <div class="dr-hint" id="drHint" data-i18n="dr_hint_start"></div>
      <div class="dr-grid" id="calGrid"></div>
      <div class="dr-footer">
        <button class="btn-clear" onclick="clearDateRange()" data-i18n="dr_clear"></button>
        <button onclick="closeCal()" data-i18n="dr_apply"></button>
      </div>
    </div>
  </div>
  <span class="sep"></span>
  <select id="fproj" onchange="applyFilter()">
    <option value="" data-i18n="all_projects">All projects</option>
    {project_options}
  </select>
  <select id="fagent" onchange="applyFilter()">
    <option value="" data-i18n="all_agents">All agents</option>
    {agent_options}
  </select>
  <button onclick="exportCsv()" data-i18n="btn_csv">⬇ CSV</button>
</div>

<!-- ── KPI Cards ── -->
<div class="cards" id="kpiCards"></div>

<!-- ── Legend ── -->
<div class="legend" style="margin-bottom:18px;">
  <span><span class="dot" style="background:var(--grn)"></span><span data-i18n="legend_good">≥20% — good</span></span>
  <span><span class="dot" style="background:var(--ylw)"></span><span data-i18n="legend_ok">10–20% — ok</span></span>
  <span><span class="dot" style="background:var(--red)"></span><span data-i18n="legend_low">&lt;10% — low</span></span>
</div>

<!-- ── Charts ── -->
<div class="charts">
  <div class="cbox"><h3 data-i18n="chart_daily_title">Daily Token Usage</h3><canvas id="dailyChart"></canvas></div>
  <div class="cbox"><h3 data-i18n="chart_fidelity_title">Daily Reduction by Fidelity (%)</h3><canvas id="fidelityChart"></canvas></div>
  <div class="cbox"><h3 data-i18n="chart_tok_trend_title">Daily Input vs Output Tokens</h3><canvas id="tokTrendChart"></canvas></div>
  <div class="cbox"><h3 data-i18n="chart_agent_title">Calls by Agent</h3><canvas id="agentChart"></canvas></div>
  <div class="cbox"><h3 data-i18n="chart_hourly_title">Hourly Pattern</h3><canvas id="hourlyChart"></canvas></div>
  <div class="cbox"><h3 data-i18n="chart_hist_title">Reduction Distribution</h3><canvas id="histChart"></canvas></div>
  <div class="cbox"><h3 data-i18n="chart_project_title">Calls by Project</h3><canvas id="projectChart"></canvas></div>
</div>

<!-- ── Daily summary ── -->
<section>
  <h2 data-i18n="sec_daily">Daily Summary</h2>
  <table><thead><tr>
    <th data-i18n="col_date">date</th><th data-i18n="col_calls">calls</th>
    <th data-i18n="col_input">input tok</th><th data-i18n="col_saved">saved</th><th>reduction</th>
  </tr></thead><tbody id="dailyTbody"></tbody></table>
</section>

<!-- ── All records ── -->
<section>
  <h2 data-i18n="sec_records">All Records</h2>
  <div style="display:flex;gap:8px;margin-bottom:12px;align-items:center;">
    <input type="text" id="ftxt" placeholder="Filter file…">
  </div>
  <table id="tbl"><thead><tr>
    <th data-i18n="col_date">date</th><th data-i18n="col_project">project</th>
    <th data-i18n="col_agent">agent</th><th data-i18n="col_file">file</th>
    <th data-i18n="col_fidelity">fidelity</th><th data-i18n="col_intok">in tok</th>
    <th>reduction</th><th data-i18n="col_saved">saved</th>
  </tr></thead><tbody id="tbody"></tbody></table>
</section>
</div>

<script>
// ── Raw data ─────────────────────────────────────────────────────────────────
const ALL={records_json};

// ── i18n ─────────────────────────────────────────────────────────────────────
const I18N = {{
  en: {{
    lbl_from:'From',lbl_to:'To',
    dr_placeholder:'Date range',dr_hint_start:'Select start date',dr_hint_end:'Select end date',
    dr_clear:'Reset',dr_apply:'Apply',
    preset_today:'Today',
    kpi_calls:'Total Calls',kpi_saved:'Tokens Saved',kpi_reduction:'Avg Reduction',
    kpi_files:'Unique Files',kpi_agents:'Agents',kpi_days:'Active Days',
    kpi_period:'in selected period',kpi_vs_input:'vs input',
    kpi_across:'across all files',kpi_processed:'processed',
    kpi_active:'active',kpi_in_range:'in range',
    legend_good:'≥20% — good',legend_ok:'10–20% — ok',legend_low:'<10% — low',
    chart_daily_title:'Daily Token Usage (input vs saved)',
    chart_fidelity_title:'Daily Reduction by Fidelity (%)',
    chart_tok_trend_title:'Daily Input vs Output Tokens',
    chart_agent_title:'Calls by Agent',
    chart_hourly_title:'Hourly Usage Pattern',
    chart_hist_title:'Reduction % Distribution',
    chart_project_title:'Calls by Project',
    sec_daily:'Daily Summary',sec_records:'All Records',
    col_date:'date',col_calls:'calls',col_input:'input tok',col_saved:'saved',
    col_project:'project',col_agent:'agent',col_file:'file',
    col_fidelity:'fidelity',col_intok:'in tok',
    all_projects:'All projects',all_agents:'All agents',btn_csv:'⬇ CSV',
    hdr_meta:'{{n}} calls · {{d}} days',
  }},
  ko: {{
    lbl_from:'시작',lbl_to:'종료',
    dr_placeholder:'날짜 범위',dr_hint_start:'시작일을 선택하세요',dr_hint_end:'종료일을 선택하세요',
    dr_clear:'초기화',dr_apply:'확인',
    preset_today:'오늘',
    kpi_calls:'총 호출',kpi_saved:'절약 토큰',kpi_reduction:'평균 압축률',
    kpi_files:'고유 파일',kpi_agents:'에이전트',kpi_days:'활용 일수',
    kpi_period:'선택 기간',kpi_vs_input:'입력 대비',
    kpi_across:'전체 파일',kpi_processed:'처리됨',
    kpi_active:'활성',kpi_in_range:'범위 내',
    legend_good:'≥20% — 양호',legend_ok:'10~20% — 보통',legend_low:'<10% — 낮음',
    chart_daily_title:'일별 토큰 사용량 (입력 vs 절약)',
    chart_fidelity_title:'일별 충실도별 압축률 (%)',
    chart_tok_trend_title:'일별 입력 vs 출력 토큰',
    chart_agent_title:'에이전트별 호출 수',
    chart_hourly_title:'시간대별 사용 패턴',
    chart_hist_title:'압축률 분포',
    chart_project_title:'프로젝트별 호출 수',
    sec_daily:'일별 요약',sec_records:'전체 기록',
    col_date:'날짜',col_calls:'호출',col_input:'입력 토큰',col_saved:'절약',
    col_project:'프로젝트',col_agent:'에이전트',col_file:'파일',
    col_fidelity:'충실도',col_intok:'입력 토큰',
    all_projects:'전체 프로젝트',all_agents:'전체 에이전트',btn_csv:'⬇ CSV 내보내기',
    hdr_meta:'{{n}}회 호출 · {{d}}일',
  }},
}};

let LANG='en';
function t(k){{return(I18N[LANG]||I18N.en)[k]||k;}}
function applyLang(){{
  document.documentElement.setAttribute('lang',LANG);
  document.querySelectorAll('[data-i18n]').forEach(el=>{{el.textContent=t(el.getAttribute('data-i18n'));}});
  document.getElementById('langBtn').textContent=LANG==='en'?'한국어':'English';
}}
function toggleLang(){{LANG=LANG==='en'?'ko':'en';applyLang();render();}}
function toggleTheme(){{
  const r=document.documentElement;
  const cur=r.getAttribute('data-theme')||'dark';
  const next=cur==='dark'?'light':'dark';
  r.setAttribute('data-theme',next);
  document.getElementById('themeBtn').textContent=next==='dark'?'☀':'🌙';
  localStorage.setItem('stats-theme',next);
  render();
}}
(function(){{const s=localStorage.getItem('stats-theme');if(s){{document.documentElement.setAttribute('data-theme',s);document.getElementById('themeBtn').textContent=s==='dark'?'☀':'🌙';}}}})();
if(navigator.language&&navigator.language.startsWith('ko')){{LANG='ko';}}
applyLang();

// ── Helpers ──────────────────────────────────────────────────────────────────
function fmtDate(d){{return d.toISOString().substring(0,10);}}
function parseD(s){{const p=s.split('-');return new Date(+p[0],+p[1]-1,+p[2]);}}
function updateDrDisplay(){{
  const f=document.getElementById('ffrom').value;
  const tv=document.getElementById('fto').value;
  const el=document.getElementById('drDisplay');
  el.textContent=f&&tv?`${{f}} ~ ${{tv}}`:t('dr_placeholder');
}}
function setPreset(btn){{
  document.querySelectorAll('.preset').forEach(b=>b.classList.remove('active'));
  btn.classList.add('active');
  const days=parseInt(btn.getAttribute('data-days'));
  const now=new Date();
  const to=fmtDate(now);
  const from=new Date(now);from.setDate(from.getDate()-days+1);
  document.getElementById('ffrom').value=fmtDate(from);
  document.getElementById('fto').value=to;
  updateDrDisplay();
  applyFilter();
}}

// ── Date range calendar ──────────────────────────────────────────────────────
let calMonth=new Date().getMonth(),calYear=new Date().getFullYear();
let pickPhase=0; // 0=pick start, 1=pick end (start selected, waiting for end)
let hoverDate=null;

function toggleCal(){{
  const c=document.getElementById('drCal');
  const isOpen=c.classList.contains('open');
  if(isOpen){{closeCal();}}else{{
    pickPhase=0;hoverDate=null;
    document.getElementById('drDisplay').classList.add('picking');
    c.classList.add('open');
    drawCal();
  }}
}}
function closeCal(){{
  document.getElementById('drCal').classList.remove('open');
  document.getElementById('drDisplay').classList.remove('picking');
  pickPhase=0;hoverDate=null;
}}
function clearDateRange(){{
  document.getElementById('ffrom').value='';
  document.getElementById('fto').value='';
  pickPhase=0;hoverDate=null;
  updateDrDisplay();
  drawCal();
  applyFilter();
}}
function calNav(dir){{
  calMonth+=dir;
  if(calMonth<0){{calMonth=11;calYear--;}}else if(calMonth>11){{calMonth=0;calYear++;}}
  drawCal();
}}
function drawCal(){{
  const mn=new Intl.DateTimeFormat(LANG==='ko'?'ko':'en',{{month:'long'}});
  document.getElementById('calTitle').textContent=`${{calYear}} ${{mn.format(new Date(calYear,calMonth,1))}}`;
  document.getElementById('drHint').textContent=pickPhase===0?t('dr_hint_start'):t('dr_hint_end');
  const grid=document.getElementById('calGrid');
  const dn=['Su','Mo','Tu','We','Th','Fr','Sa'];
  let html=dn.map(d=>`<div class="dh">${{d}}</div>`).join('');
  const startDay=new Date(calYear,calMonth,1).getDay();
  const daysInMonth=new Date(calYear,calMonth+1,0).getDate();
  const prevDays=new Date(calYear,calMonth,0).getDate();
  const todayStr=fmtDate(new Date());
  for(let i=0;i<startDay;i++)html+=`<div class="dc out">${{prevDays-startDay+i+1}}</div>`;
  for(let d=1;d<=daysInMonth;d++){{
    const ds=`${{String(calYear).padStart(4,'0')}}-${{String(calMonth+1).padStart(2,'0')}}-${{String(d).padStart(2,'0')}}`;
    let cls='dc';
    if(ds===todayStr)cls+=' today';
    html+=`<div class="${{cls}}" data-date="${{ds}}">${{d}}</div>`;
  }}
  const total=startDay+daysInMonth;
  const rem=total%7===0?0:7-total%7;
  for(let i=1;i<=rem;i++)html+=`<div class="dc out">${{i}}</div>`;
  grid.innerHTML=html;
  updateCalStyles();
}}
function updateCalStyles(){{
  const from=document.getElementById('ffrom').value;
  const to=document.getElementById('fto').value;
  // phase 1: 시작일 고정, hover로 end 미리보기
  const end=pickPhase===1?(hoverDate||from):to;
  const lo=from&&end?(from<=end?from:end):'';
  const hi=from&&end?(from<=end?end:from):'';
  document.querySelectorAll('#calGrid .dc[data-date]').forEach(el=>{{
    const ds=el.getAttribute('data-date');
    el.classList.remove('sel-start','sel-end','range','preview');
    if(!lo)return;
    if(ds===lo&&ds===hi){{el.classList.add('sel-start','sel-end');}}
    else if(ds===lo){{el.classList.add('sel-start');}}
    else if(ds===hi){{el.classList.add('sel-end');}}
    else if(ds>lo&&ds<hi){{el.classList.add(pickPhase===1?'preview':'range');}}
  }});
}}
// calGrid 이벤트 위임 — stopPropagation으로 document 핸들러 차단
document.getElementById('calGrid').addEventListener('click',e=>{{
  e.stopPropagation();
  const dc=e.target.closest('.dc[data-date]');
  if(dc)pickDate(dc.getAttribute('data-date'));
}});
document.getElementById('calGrid').addEventListener('mouseover',e=>{{
  const dc=e.target.closest('.dc[data-date]');
  if(dc&&pickPhase===1){{hoverDate=dc.getAttribute('data-date');updateCalStyles();}}
}});
// 외부 클릭 시 달력 닫기
document.addEventListener('click',e=>{{
  if(!e.target.closest('.dr')&&document.getElementById('drCal').classList.contains('open'))closeCal();
}});
function pickDate(ds){{
  if(pickPhase===0){{
    document.getElementById('ffrom').value=ds;
    document.getElementById('fto').value='';
    pickPhase=1;
    hoverDate=null;
  }}else{{
    const from=document.getElementById('ffrom').value;
    if(ds<from){{document.getElementById('ffrom').value=ds;document.getElementById('fto').value=from;}}
    else if(ds===from){{document.getElementById('fto').value=ds;}}
    else{{document.getElementById('fto').value=ds;}}
    pickPhase=0;
    hoverDate=null;
    // 달력을 닫지 않음 — 사용자가 확인 버튼 또는 외부 클릭으로 닫음
  }}
  syncPresets();
  updateDrDisplay();
  drawCal();
  applyFilter();
}}
function syncPresets(){{
  const from=document.getElementById('ffrom').value;
  const tv=document.getElementById('fto').value;
  const now=new Date();const today=fmtDate(now);
  document.querySelectorAll('.preset').forEach(b=>{{
    const days=parseInt(b.getAttribute('data-days'));
    const d=new Date(now);d.setDate(d.getDate()-days+1);
    b.classList.toggle('active',fmtDate(d)===from&&today===tv);
  }});
}}
const G={{color:'rgba(128,128,128,.08)'}},F={{color:'var(--mut)'}};
function getChartTextColor(){{return getComputedStyle(document.documentElement).getPropertyValue('--mut').trim()||'#8892a4';}}
function getChartGridColor(){{return getComputedStyle(document.documentElement).getPropertyValue('--bdr').trim()||'rgba(128,128,128,.15)';}}
const AC=['#6366f1','#22c55e','#eab308','#ef4444','#06b6d4','#f97316','#a855f7','#ec4899'];
function pcls(p){{return p>=20?'good':p>=10?'ok':'low';}}
function esc(s){{const d=document.createElement('div');d.textContent=s;return d.innerHTML;}}
function fmt(n){{return n.toString().replace(/\B(?=(\d{{3}})+(?!\d))/g,' ');}}
function dateOf(ts){{return ts.substring(0,10);}}
function hourOf(ts){{const p=ts.split('T')[1];return p?parseInt(p.substring(0,2),10):-1;}}
function r2(v){{return Math.round(v*100)/100;}}
function recProj(r){{return r.project||'unknown';}}
function recAgent(r){{return r.agent||'unknown';}}

// ── Chart instances ──────────────────────────────────────────────────────────
let charts={{}};
function mkChart(id,cfg){{if(charts[id]){{charts[id].destroy();}}charts[id]=new Chart(document.getElementById(id),cfg);}}

// ── Render everything from filtered data ─────────────────────────────────────
function render(){{
  const tc=getChartTextColor(),gc=getChartGridColor();
  const F={{color:tc}},G={{color:gc}};
  const from=document.getElementById('ffrom').value;
  const to=document.getElementById('fto').value;
  const proj=document.getElementById('fproj').value;
  const agent=document.getElementById('fagent').value;
  const txt=document.getElementById('ftxt').value.toLowerCase();

  const recs=ALL.filter(r=>{{
    const d=dateOf(r.ts);
    const rp=recProj(r),ra=recAgent(r);
    return(!from||d>=from)&&(!to||d<=to)&&(!proj||rp===proj)&&(!agent||ra===agent)&&(!txt||r.file.toLowerCase().includes(txt));
  }});

  // KPI
  const calls=recs.length;
  const inputTok=recs.reduce((s,r)=>s+r.input_tok,0);
  const savedTok=recs.reduce((s,r)=>s+r.saved,0);
  const avgRed=inputTok>0?r2(savedTok/inputTok*100):0;
  const uniqFiles=new Set(recs.map(r=>r.file)).size;
  const uniqAgents=new Set(recs.map(recAgent)).size;
  const uniqDays=new Set(recs.map(r=>dateOf(r.ts))).size;

  document.getElementById('hdrMeta').textContent=t('hdr_meta').replace('{{n}}',calls).replace('{{d}}',uniqDays);
  document.getElementById('kpiCards').innerHTML=[
    ['kpi_calls',calls,'var(--acc)','kpi_period'],
    ['kpi_saved',fmt(savedTok),'var(--grn)','kpi_vs_input',fmt(inputTok)],
    ['kpi_reduction',avgRed.toFixed(1)+'%','var(--grn)','kpi_across'],
    ['kpi_files',uniqFiles,'var(--txt)','kpi_processed'],
    ['kpi_agents',uniqAgents,'var(--txt)','kpi_active'],
    ['kpi_days',uniqDays,'var(--txt)','kpi_in_range'],
  ].map(([lbl,val,col,sub,subVal])=>`<div class="card"><div class="lbl">${{t(lbl)}}</div><div class="val" style="color:${{col}}">${{val}}</div><div class="sub">${{t(sub).replace('input',subVal||'')}}</div></div>`).join('');

  // Daily aggregates
  const dailyMap={{}};
  const dailySem={{}},dailyCmp={{}};
  recs.forEach(r=>{{
    const d=dateOf(r.ts);
    dailyMap[d]??={{c:0,i:0,o:0,s:0}};const e=dailyMap[d];
    e.c++;e.i+=r.input_tok;e.o+=r.output_tok;e.s+=r.saved;
    if(r.fidelity==='semantic'){{dailySem[d]??={{s:0,c:0}};const s=dailySem[d];s.s+=r.reduction_pct;s.c++;}}
    if(r.fidelity==='compressed'){{dailyCmp[d]??={{s:0,c:0}};const s=dailyCmp[d];s.s+=r.reduction_pct;s.c++;}}
  }});
  const days=Object.keys(dailyMap).sort();
  const dInput=days.map(d=>dailyMap[d].i);
  const dOutput=days.map(d=>dailyMap[d].o);
  const dSaved=days.map(d=>dailyMap[d].s);
  const dSemPct=days.map(d=>{{const x=dailySem[d];return x?r2(x.s/x.c):null;}});
  const dCmpPct=days.map(d=>{{const x=dailyCmp[d];return x?r2(x.s/x.c):null;}});

  // Agent / Project counts
  const agentMap={{}},projMap={{}},hourly=new Array(24).fill(0);
  const histB=['<0%','0-5%','5-10%','10-20%','20-30%','30-50%','50%+'];
  const histV=[0,0,0,0,0,0,0];
  const histBounds=[[-Infinity,0],[0,5],[5,10],[10,20],[20,30],[30,50],[50,Infinity]];
  recs.forEach(r=>{{
    const ra=recAgent(r),rp=recProj(r);
    agentMap[ra]=(agentMap[ra]||0)+1;
    projMap[rp]=(projMap[rp]||0)+1;
    const h=hourOf(r.ts);if(h>=0&&h<24)hourly[h]++;
    for(let i=0;i<histBounds.length;i++){{if(r.reduction_pct>=histBounds[i][0]&&r.reduction_pct<histBounds[i][1]){{histV[i]++;break;}}}}
  }});
  const aLabels=Object.keys(agentMap).sort(),aData=aLabels.map(k=>agentMap[k]);
  const pLabels=Object.keys(projMap).sort((a,b)=>projMap[b]-projMap[a]),pData=pLabels.map(k=>projMap[k]);

  // ── Charts ──
  mkChart('dailyChart',{{type:'bar',data:{{labels:days,datasets:[
    {{label:'Input',data:dInput,backgroundColor:'rgba(99,102,241,.5)',borderRadius:3,stack:'a'}},
    {{label:'Output',data:dOutput,backgroundColor:'rgba(34,197,94,.6)',borderRadius:3,stack:'a'}},
  ]}},options:{{plugins:{{legend:{{labels:{{color:tc}}}}}},scales:{{x:{{ticks:F,grid:G,stacked:true}},y:{{ticks:F,grid:G,stacked:true,title:{{display:true,text:'tokens',color:tc}}}}}}}}}});

  mkChart('fidelityChart',{{type:'line',data:{{labels:days,datasets:[
    {{label:'Semantic %',data:dSemPct,borderColor:'#22c55e',backgroundColor:'rgba(34,197,94,.08)',tension:.3,pointRadius:4,spanGaps:true}},
    {{label:'Compressed %',data:dCmpPct,borderColor:'#ef4444',backgroundColor:'rgba(239,68,68,.08)',tension:.3,pointRadius:4,spanGaps:true}},
  ]}},options:{{plugins:{{legend:{{labels:{{color:tc}}}}}},scales:{{x:{{ticks:F,grid:G}},y:{{ticks:F,grid:G,title:{{display:true,text:'reduction %',color:tc}}}}}}}}}});

  mkChart('tokTrendChart',{{type:'line',data:{{labels:days,datasets:[
    {{label:'Input',data:dInput,borderColor:'#6366f1',backgroundColor:'rgba(99,102,241,.1)',fill:true,tension:.3,pointRadius:3}},
    {{label:'Output',data:dOutput,borderColor:'#22c55e',backgroundColor:'rgba(34,197,94,.1)',fill:true,tension:.3,pointRadius:3}},
  ]}},options:{{plugins:{{legend:{{labels:{{color:tc}}}}}},scales:{{x:{{ticks:F,grid:G}},y:{{ticks:F,grid:G,title:{{display:true,text:'tokens',color:tc}}}}}}}}}});

  mkChart('agentChart',{{type:'doughnut',data:{{labels:aLabels,datasets:[{{data:aData,backgroundColor:AC.slice(0,aLabels.length),borderWidth:2}}]}},
    options:{{plugins:{{legend:{{labels:{{color:tc,padding:14}}}}}},cutout:'55%'}}}});

  mkChart('hourlyChart',{{type:'bar',data:{{labels:Array.from({{length:24}},(_,i)=>String(i).padStart(2,'0')),
    datasets:[{{data:hourly,backgroundColor:'rgba(99,102,241,.6)',borderRadius:3}}]}},
    options:{{plugins:{{legend:{{display:false}}}},scales:{{
      x:{{ticks:F,grid:G,title:{{display:true,text:'hour (UTC)',color:tc}}}},
      y:{{ticks:{{...F,stepSize:1}},grid:G,title:{{display:true,text:'calls',color:tc}}}}
    }}}}}});

  mkChart('histChart',{{type:'bar',data:{{labels:histB,datasets:[{{data:histV,backgroundColor:'rgba(34,197,94,.6)',borderRadius:3}}]}},
    options:{{plugins:{{legend:{{display:false}}}},scales:{{
      x:{{ticks:F,grid:G,title:{{display:true,text:'reduction %',color:tc}}}},
      y:{{ticks:{{...F,stepSize:1}},grid:G,title:{{display:true,text:'count',color:tc}}}}
    }}}}}});

  mkChart('projectChart',{{type:'bar',data:{{labels:pLabels,datasets:[{{data:pData,backgroundColor:'rgba(234,179,8,.6)',borderRadius:3}}]}},
    options:{{indexAxis:'y',plugins:{{legend:{{display:false}}}},scales:{{
      x:{{ticks:F,grid:G,title:{{display:true,text:'calls',color:tc}}}},y:{{ticks:F,grid:G}}
    }}}}}});

  // ── Daily table ──
  document.getElementById('dailyTbody').innerHTML=days.map(d=>{{
    const e=dailyMap[d];const pct=e.i>0?r2(e.s/e.i*100):0;
    return `<tr><td>${{esc(d)}}</td><td class="n">${{e.c}}</td><td class="n">${{fmt(e.i)}}</td><td class="n">${{fmt(e.s)}}</td><td class="n ${{pcls(pct)}}">${{pct.toFixed(1)}}%</td></tr>`;
  }}).join('');

  // ── All records table ──
  document.getElementById('tbody').innerHTML=recs.map(r=>{{
    const pct=r.reduction_pct;
    return `<tr><td>${{esc(dateOf(r.ts))}}</td><td>${{esc(recProj(r))}}</td><td>${{esc(recAgent(r))}}</td><td>${{esc(r.file)}}</td><td>${{esc(r.fidelity)}}</td><td class="n">${{r.input_tok}}</td><td class="n ${{pcls(pct)}}">${{pct.toFixed(1)}}%</td><td class="n">${{r.saved}}</td></tr>`;
  }}).join('');
}}

function applyFilter(){{
  syncPresets();
  render();
}}

function exportCsv(){{
  const hdr=['date','project','agent','file','fidelity','in_tok','reduction','saved'];
  const rows=[hdr];
  document.querySelectorAll('#tbody tr').forEach(row=>{{
    if(row.style.display==='none')return;
    rows.push([...row.querySelectorAll('td')].map(td=>td.textContent));
  }});
  const a=document.createElement('a');
  a.href=URL.createObjectURL(new Blob([rows.map(r=>r.join(',')).join('\n')],{{type:'text/csv'}}));
  a.download='stats.csv';a.click();
}}

// ftxt debounce — 200ms
(function(){{
  let _t;
  document.getElementById('ftxt').addEventListener('input',()=>{{
    clearTimeout(_t);_t=setTimeout(applyFilter,200);
  }});
}})();

// Initial render — default to 1 week
(function(){{
  const now=new Date();
  const from=new Date(now);from.setDate(from.getDate()-6);
  document.getElementById('ffrom').value=fmtDate(from);
  document.getElementById('fto').value=fmtDate(now);
}})();
render();
</script>
</body>
</html>
"##,
        records_json = records_json,
        project_options = project_options,
        agent_options = agent_options,
    );

    if let Err(e) = fs::write(out_path, &html) {
        eprintln!("ERROR: cannot write HTML to {out_path}: {e}");
        std::process::exit(1);
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_prevents_xss() {
        let s = esc("<img onerror=alert(1)>");
        assert!(!s.contains('<'));
        assert!(s.contains("&lt;"));
    }

    #[test]
    fn js_safe_escapes_script_close() {
        assert_eq!(js_safe("</script>"), "<\\/script>");
    }

    #[test]
    fn deserialize_stats_record_without_project() {
        let json = r#"{"ts":"2026-05-12T00:00:00Z","agent":"claude","file":"a.rs","format":"markdown","fidelity":"semantic","input_tok":100,"output_tok":80,"reduction_pct":20.0,"saved":20}"#;
        let rec: StatsRecord = serde_json::from_str(json).unwrap();
        assert_eq!(rec.project, "");
        assert_eq!(rec.agent, "claude");
        assert_eq!(rec.saved, 20);
    }

    #[test]
    fn deserialize_stats_record_with_project() {
        let json = r#"{"ts":"2026-05-12T00:00:00Z","agent":"claude","project":"llm-transpile","file":"a.rs","format":"markdown","fidelity":"semantic","input_tok":100,"output_tok":80,"reduction_pct":20.0,"saved":20}"#;
        let rec: StatsRecord = serde_json::from_str(json).unwrap();
        assert_eq!(rec.project, "llm-transpile");
    }

    #[test]
    fn epoch_days_to_ymd_epoch() {
        assert_eq!(epoch_days_to_ymd(0), (1970, 1, 1));
    }
}
