# 實作進度

## 2026-11-26 Review 與驗證

- 接手時遠端 `develop` 為 `b409aceb`，功能分支為 `a3ee9cc`，領先 17 個提交且落後 0 個。已以功能分支建立指向 `develop` 的 [草稿 PR #1](https://github.com/DevinKWu/shawl/pull/1)。
- Rust stable 1.100.0 上，`029d0ef` 通過 Linux 的 `cargo fmt --all --check`、`cargo check --locked --all-targets`、`cargo clippy --locked --all-targets -- -D warnings`、`cargo test --locked`（58 個）和 `cargo build --locked`。另以 systemd 257 執行 Rust 產出之 user unit 語法測試（1 個），以及 `actionlint`、`bash -n tests/native-smoke.sh` 和 `git diff --check`。
- macOS `aarch64-apple-darwin`、Windows `x86_64-pc-windows-gnu` 和 `i686-pc-windows-gnu` 均通過 `cargo check` 與嚴格 Clippy（含測試目標）。macOS 的 3 個純 Rust plist 測試亦在 Linux 隔離測試專案通過。Windows SCM、Job Object 原生回歸及 macOS `plutil`、launchd 原生測試仍待執行。
- RustSec 稽核確認 `crossbeam-epoch` 0.9.18 命中 [RUSTSEC-2026-0204](https://rustsec.org/advisories/RUSTSEC-2026-0204)，鎖定至 0.9.20 後重跑，未發現已知漏洞。現有 `chrono` 有停止維護警告，`speculate` 的 `syn` 0.14.9 有 Rust 未來相容性警告。
- 獨立 Debian 13 VM（systemd 257.13）使用 `029d0ef` 通過 system 與 user scope 的 `tests/native-smoke.sh`。驗證包括特殊路徑、完整 argv/env/cwd、非 root UID、檔案權限、重複拒絕、異常重啟、停止、journal 和子程序清理。另確認 user scope 的載入路徑錯配及有效 `Type` 不符皆會拒絕並移除註冊檔。
- Debian VM 已驗證 system scope 與啟用 lingering 的 user scope，在 `systemctl enable` 後於新 boot ID 下各自啟動一次，且服務 UID 正確。
- 功能分支推送及開啟 PR 後，GitHub API 仍回報 0 個 Actions 工作流程、0 筆執行。啟用 Main 工作流程回傳 404，查詢及啟用儲存庫 Actions 設定均回傳 403（`Resource not accessible by integration`）。需要儲存庫管理者啟用 Actions 後，確認三平台的原生執行結果。PR 維持草稿。

## 接手前紀錄

### 已完成（當時尚未編譯驗證）
- 分支建立，實作開始時確認未落後 develop；保留原有 Windows 服務實作。
- Windows 相依及 build resource 分離，既有 CLI 平台分派。
- Linux systemd user/system 服務定義產生、註冊、載入檔案驗證與失敗清理。
- macOS launchd gui/system plist 產生、註冊、plutil 驗證；原生管理指令由管理者操作。
- Unix 系統級註冊須指定非 root 服務帳號，共用安全檔案建立與名稱驗證。
- 新增單元測試（scope、CLI 帳號參數互相依賴、引數/XML escaping、環境變數、重複檔案防護與重啟策略），更新平台文件與三平台 CI test matrix。
- 新增原生設定語法測試（systemd-analyze/plutil 驗證 Rust 實際產出的檔案），CI 在 Linux/macOS 明確執行；不註冊或啟動服務。

- Review 修正：原子發布服務檔案且不覆寫檔案/符號連結、macOS 先 lint 再發布並檢查標準目錄未載入 label、systemd system scope 清除繼承環境並確認實際 User 屬性；補齊相應測試。

- 新增 tests/native-smoke.sh，供 Linux/macOS user/system scope 驗收；既有 hosted CI 測試矩陣新增 system scope smoke，不新增 workflow 或工作。

### 當時的驗證與限制
- 本次容器確實無 cargo/rustc；未編譯或執行 Rust 測試。本次本地 systemd 257 對代表性系統級 unit 執行 verify，exit 0（測試用 nobody 帳號有安全警告）；並非 Rust 產出或原生整合驗證。新加入的語法測試尚未執行。
- 本輪 HEAD 56a308548154937689af239f410ea5057122fb99 的 Actions 執行數為 0；容器再次確認無 cargo/rustc，且無法解析 GitHub 與 Rust 下載站 DNS。尚無三平台測試成功證據。
- 本輪代表性 user unit 以本機 systemd 257 verify 返回 0；代表性 plist 以 Linux 上的 Swift plutil -lint 返回 0，Python plistlib 反序列化保留特殊字元及空引數。並非 Rust 產出、macOS 原生驗收或服務生命週期測試。
- macOS 系統級服務初版需要 --no-log 或 --no-log-cmd；使用者日誌無內建輪替；兩平台無法等價映射的選項明確拒絕。非標準目錄未載入的 launchd plist 不在衝突檢查範圍。
- Windows 原有 SCM/Job Object 程式碼未修改，但回歸測試仍須執行。

- smoke 腳本已本機執行 bash -n、無參數拒絕（exit 2）與獨立 fixture 的 argv/env/cwd/UID、SIGTERM 測試（exit 0）；並非原生服務註冊測試。尚未執行完整 smoke 或 CI。

### 接手前的待辦
- 確認 GitHub Fork Actions 啟用狀態與三平台 CI 結果，修正實際編譯、格式及測試問題。
- Linux/macOS 原生 user/system 實機整合驗收：參數保真、服務帳號、重複註冊、重啟、停機、日誌、開機/登入載入與失敗清理。
- 補齊確有必要的原生差異處理，不新增 TOML/.env/mise 或統一 CRUD CLI。
