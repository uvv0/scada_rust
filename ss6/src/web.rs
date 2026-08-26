const BUILD_ID: &str = env!("SS6_BUILD_ID");
const BUILD_LABEL: &str = concat!("v", env!("CARGO_PKG_VERSION"), " build ", env!("SS6_BUILD_ID"));

const INDEX_HTML_TEMPLATE: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>ss6 __BUILD_LABEL__ - KPZ/Group/Reg selector</title>
  <link rel="stylesheet" href="/static/app.css?v=__ASSET_VERSION__">
</head>
<body>
  <div class="wrap">
    <div class="panel">
      <div class="top-bar">
        <div class="seg-control" id="modeControl">
          <button class="seg-btn seg-btn--active" id="modeChartBtn">Charts</button>
          <button class="seg-btn" id="modePreviewBtn">UI Preview</button>
        </div>
        <div class="top-bar__right">
          <span class="top-bar__build">__BUILD_LABEL__</span>
          <span class="top-bar__boot" id="bootStatus">Boot: HTML loaded.</span>
          <button id="diagToggle" class="btn-secondary" style="font-size:11px;">Diag</button>
          <button id="reloadBtn" class="btn-secondary">Reload</button>
          <button id="logoutBtn" class="btn-secondary">Logout</button>
        </div>
      </div>
      <div class="diag-panel" id="diagPanel">
        <div id="diagContent">Loading diagnostics...</div>
      </div>
      <div class="layout">
        <button class="left-panel-toggle btn-secondary" id="leftPanelToggle">Show controls</button>
        <div class="left-panel" id="leftPanel">
          <div class="left-stack">
            <div class="surface">
              <div class="surface__head">
                <div class="surface__title">KPZ</div>
                <div class="surface__meta">Search and choose the active object</div>
              </div>
              <div class="surface__actions">
                <div class="action-group action-group--grow">
                  <div class="action-field">
                    <label for="kpzSearch">Search</label>
                    <input id="kpzSearch" placeholder="Search KPZ..." />
                  </div>
                  <div class="action-field">
                    <label for="kpz">KPZ</label>
                    <select id="kpz"></select>
                  </div>
                </div>
              </div>
              <div class="surface__status" id="kpzDebug"></div>
            </div>
            <div class="surface">
              <div class="surface__head">
                <div class="surface__title">Group</div>
                <div class="surface__meta">Filter the reg list by group</div>
              </div>
              <div class="surface__actions">
                <div class="action-group action-group--grow">
                  <div class="action-field">
                    <label for="group">Group</label>
                    <select id="group"></select>
                  </div>
                </div>
              </div>
            </div>
            <div class="surface">
              <div class="surface__head">
                <div class="surface__title">Regs</div>
                <div class="surface__meta">Select regs for charts and preview context</div>
              </div>
              <div class="surface__actions">
                <div class="action-group action-group--grow">
                  <div class="action-field">
                    <label for="regSearch">Search</label>
                    <input id="regSearch" placeholder="Filter regs by name or id..." />
                  </div>
                </div>
              </div>
              <div class="reg-controls">
                <button id="regSelectVisible">Select visible</button>
                <button id="regClear">Clear</button>
                <button id="regOnlySelected">Only selected</button>
              </div>
              <div class="reg-chips" id="selectedChips"></div>
              <div class="regs" id="regs"></div>
            </div>
          </div>
        </div>
        <div class="right-panel">
          <div class="surface" id="chartBlock">
            <div class="surface__head">
              <div class="surface__title">Charts</div>
              <div class="surface__meta">ARX series by selected regs</div>
            </div>
              <div class="surface__chips">
                <span class="chip" id="chartModeChip" data-tone="active">Mode: Charts</span>
                <span class="chip" id="chartSourceChip" data-tone="muted">Source: idle</span>
                <span class="chip" id="chartSelectionChip" data-tone="muted">Selection: 0</span>
                <span class="chip" id="chartQualityChip" data-tone="muted">Quality: -</span>
                <span class="chip" id="chartVisibleChip" data-tone="muted">Rows: 0</span>
                <span class="chip" id="chartPointsChip" data-tone="muted">Points: 0</span>
                <span class="chip" id="chartTimeChip" data-tone="muted">Last time: -</span>
                <span class="chip" id="chartLastChip" data-tone="muted">Last: -</span>
                <span class="chip" id="chartRangeChip" data-tone="muted">Range: -</span>
              </div>
              <div class="surface__actions">
                <div class="action-group action-group--grow">
                  <div class="action-field">
                    <label for="chartPreset">Chart preset</label>
                    <select id="chartPreset">
                      <option value="operator" selected>Operator</option>
                      <option value="dense">Dense</option>
                      <option value="presentation">Presentation</option>
                      <option value="custom">Custom</option>
                    </select>
                  </div>
                  <div class="action-field">
                    <label>Time window</label>
                    <div style="display:flex;gap:4px;flex-wrap:wrap;">
                      <button class="time-preset-btn" data-sec="900">15m</button>
                      <button class="time-preset-btn" data-sec="3600">1h</button>
                      <button class="time-preset-btn" data-sec="21600">6h</button>
                      <button class="time-preset-btn time-preset-btn--active" data-sec="86400">24h</button>
                      <button class="time-preset-btn" data-sec="604800">7d</button>
                    </div>
                  </div>
                </div>
                <div class="action-group">
                  <label class="check-inline"><input type="checkbox" id="chartFocusOne" />Focus one</label>
                  <label class="check-inline"><input type="checkbox" id="chartSmooth" />Smooth</label>
                  <label class="check-inline"><input type="checkbox" id="chartSymbols" />Points</label>
                </div>
                <div class="action-group">
                  <div class="action-field" style="min-width:120px;flex:0 0 120px;">
                    <label for="legendSearch">Legend search</label>
                    <input id="legendSearch" placeholder="Filter series..." />
                  </div>
                  <button id="chartShowAll">Show all</button>
                  <button id="chartHideAll">Hide all</button>
                  <button id="chartCopySummary">Copy summary</button>
                  <button id="chartCopyPng">Copy PNG</button>
                  <button id="chartExport">Export PNG</button>
                  <button id="chartCopyStatus" style="display:none;"></button>
                  <button id="show">Show</button>
                </div>
              </div>
            <div class="surface__status" id="chartStatus">Select regs and click Show to build a chart.</div>
            <div class="chart-stats" id="chartStats" style="display:none;">
              <span class="chart-stat" id="csQuality">Quality: <b>-</b></span>
              <span class="chart-stat" id="csVisible">Visible: <b>0</b></span>
              <span class="chart-stat" id="csPoints">Points: <b>0</b></span>
              <span class="chart-stat" id="csLast">Last: <b>-</b></span>
              <span class="chart-stat" id="csDelta">Delta: <b>-</b></span>
              <span class="chart-stat" id="csRange">Range: <b>-</b></span>
            </div>
            <div id="chartEmpty" class="chart-empty">
              <div class="chart-empty__icon">&#128202;</div>
              <div class="chart-empty__text">Select registers to build a chart</div>
            </div>
            <div id="chart"></div>
            <div id="chartExportStatus" style="display:none;font-size:11px;padding:4px 8px;border-radius:4px;margin-top:4px;"></div>
          </div>
          <div class="surface" id="previewBlock">
            <div class="surface__head">
              <div class="surface__title">UI Preview</div>
              <div class="surface__meta">Window layout, live values, write actions</div>
            </div>
            <div class="surface__actions">
              <div class="action-group action-group--grow">
                <div class="action-field">
                  <label for="uiWindow">UI window</label>
                  <select id="uiWindow"></select>
                </div>
              </div>
              <div class="action-group">
                <button id="pollPreviewReal">Read REAL</button>
                <button id="pollPreviewDb">Read DB</button>
              </div>
              <div class="action-group">
                <label class="check-inline"><input type="checkbox" id="autoPreviewDb" />Auto</label>
                <div class="action-field">
                  <label for="autoPreviewSource">Source</label>
                  <select id="autoPreviewSource">
                    <option value="real">REAL</option>
                    <option value="db" selected>DB</option>
                  </select>
                </div>
                <div class="action-field">
                  <label for="autoPreviewDbInterval">Interval</label>
                  <select id="autoPreviewDbInterval">
                    <option value="1000">1s</option>
                    <option value="2000" selected>2s</option>
                    <option value="5000">5s</option>
                  </select>
                </div>
              </div>
            </div>
            <div class="preview-toolbar">
              <button id="previewFitW">Fit W</button>
              <button id="previewFitH">Fit H</button>
              <button id="previewFit">Fit all</button>
              <button id="preview100">100%</button>
              <button id="previewZoomOut">-</button>
              <button id="previewZoomIn">+</button>
              <span class="spacer"></span>
              <span class="zoom-readout" id="previewZoomValue">100%</span>
              <span class="zoom-readout" id="previewStageSize" style="font-size:10px;color:#6a7a8c;"></span>
            </div>
            <div class="surface__chips">
              <span class="chip" id="previewModeChip" data-tone="muted">Mode: Charts</span>
              <span class="chip" id="previewSourceChip" data-tone="muted">Source: idle</span>
              <span class="chip" id="previewAutoChip" data-tone="muted">Auto: off</span>
              <span class="chip" id="previewWsChip" data-tone="muted">WS: off</span>
            </div>
            <div class="surface__status" id="previewStatus"></div>
            <details class="surface__debug" id="previewDebugWrap" hidden>
              <summary>Preview details</summary>
              <div class="surface__debug-body" id="previewDebug"></div>
            </details>
            <details class="action-log" id="actionLogWrap" hidden>
              <summary style="cursor:pointer;list-style:none;padding:6px 10px;font-size:12px;font-weight:600;color:#9fb0c4;user-select:none;">Action log</summary>
              <div id="actionLog"><div class="action-log__empty">No actions yet.</div></div>
            </details>
            <div id="preview"><div id="previewStageWrap"><div id="previewStage"></div></div></div>
          </div>
        </div>
      </div>
    </div>
  </div>
  <script src="https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js"></script>
  <script type="module" src="/static/api.js?v=__ASSET_VERSION__"></script>
  <script type="module" src="/static/app.js?v=__ASSET_VERSION__"></script>
</body>
</html>"#;

const LOGIN_HTML_TEMPLATE: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>ss6 __BUILD_LABEL__ login</title>
  <style>
    :root { color-scheme: dark; }
    body { margin:0; min-height:100vh; display:grid; place-items:center; background:linear-gradient(160deg,#0f1622,#182535 60%,#101826); color:#e6edf3; font-family:Segoe UI, Arial, sans-serif; }
    .card { width:min(420px, calc(100vw - 32px)); background:#182230; border:1px solid #2a3648; border-radius:14px; padding:20px; box-shadow:0 18px 44px rgba(0,0,0,.35); }
    h1 { margin:0 0 8px 0; font-size:22px; }
    p { margin:0 0 14px 0; color:#a9b9ca; }
    label { display:block; margin:12px 0 6px 0; color:#cfe0f0; }
    input, button { width:100%; box-sizing:border-box; background:#0f1722; color:#e6edf3; border:1px solid #2a3648; border-radius:10px; padding:10px 12px; }
    button { margin-top:16px; background:#21415e; cursor:pointer; font-weight:600; }
    #status { min-height:18px; margin-top:12px; font-size:13px; color:#f1c66c; }
  </style>
</head>
<body>
  <form class="card" id="loginForm">
    <h1>ss6 Web Login</h1>
    <p>Enter your credentials to access the web UI and API. <span style="color:#8fb7d9;">__BUILD_LABEL__</span></p>
    <label for="login">Login</label>
    <input id="login" name="login" autocomplete="username" />
    <label for="password">Password</label>
    <input id="password" name="password" type="password" autocomplete="current-password" />
    <button type="submit">Sign in</button>
    <div id="status"></div>
  </form>
  <script>
    const form = document.getElementById("loginForm");
    const statusEl = document.getElementById("status");
    form.addEventListener("submit", async (ev) => {
      ev.preventDefault();
      statusEl.textContent = "Signing in...";
      const login = document.getElementById("login").value.trim();
      const password = document.getElementById("password").value;
      try {
        const res = await fetch("/login", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ login, password })
        });
        const body = await res.json().catch(() => ({}));
        if (!res.ok) throw new Error(body.message || "login failed");
        window.location.href = "/";
      } catch (err) {
        statusEl.textContent = err && err.message ? err.message : String(err);
      }
    });
  </script>
</body>
</html>"#;

pub fn index_html() -> String {
    INDEX_HTML_TEMPLATE
        .replace("__BUILD_LABEL__", BUILD_LABEL)
        .replace("__ASSET_VERSION__", BUILD_ID)
}

pub fn login_html() -> String {
    LOGIN_HTML_TEMPLATE.replace("__BUILD_LABEL__", BUILD_LABEL)
}
