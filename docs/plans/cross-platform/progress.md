# 實作進度

## 已完成（已實作，尚未編譯驗證）
- 分支建立，實作開始時確認未落後 develop；保留原有 Windows 服務實作。
- Windows 相依及 build resource 分離，既有 CLI 平台分派。
- Linux systemd user/system 服務定義產生、註冊、載入檔案驗證與失敗清理。
- macOS launchd gui/system plist 產生、註冊、plutil 驗證；原生管理指令由管理者操作。
- Unix 系統級註冊須指定非 root 服務帳號，共用安全檔案建立與名稱驗證。
- 新增單元測試（scope、CLI 帳號參數互相依賴、引數/XML escaping、環境變數、重複檔案防護與重啟策略），更新平台文件與三平台 CI test matrix。
- 新增原生設定語法測試（systemd-analyze/plutil 驗證 Rust 實際產出的檔案），CI 在 Linux/macOS 明確執行；不註冊或啟動服務。

## 驗證與限制
- 本次容器確實無 cargo/rustc；未編譯或執行 Rust 測試。本次本地 systemd 257 對代表性系統級 unit 執行 verify，exit 0（測試用 nobody 帳號有安全警告）；並非 Rust 產出或原生整合驗證。新加入的語法測試尚未執行。
- 開始本次實作時，查詢 HEAD Actions 執行數為 0；尚無三平台測試成功證據。
- macOS 系統級服務初版需要 --no-log 或 --no-log-cmd；使用者日誌無內建輪替；兩平台無法等價映射的選項明確拒絕。launchd 未載入的其他 plist 標籤衝突仍待加強。
- Windows 原有 SCM/Job Object 程式碼未修改，但回歸測試仍須執行。

## 下一步
- 確認 GitHub Fork Actions 啟用狀態與三平台 CI 結果，修正實際編譯、格式及測試問題。
- Linux/macOS 原生 user/system 實機整合驗收：參數保真、服務帳號、重複註冊、重啟、停機、日誌、開機/登入載入與失敗清理。
- 補齊確有必要的原生差異處理，不新增 TOML/.env/mise 或統一 CRUD CLI。
