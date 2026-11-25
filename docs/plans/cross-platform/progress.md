# 實作進度

## 已完成（程式碼已提交，尚未編譯）
- 建立 develop 與 feature/cross-platform；確認實作開始時功能分支未落後 develop。
- 審查 CLI、main、Windows service/control/Job Object、child binary、既有測試及 CI。
- 分離 Windows 專屬相依與 API，build.rs 依目標平台決定是否編譯 resource；保留 Windows CLI 與服務實作。
- Linux 初版：systemd user unit 產生、註冊、原生管理、重複註冊防護、載入路徑驗證、失敗清理、基本驗證與單元測試；不直接啟動或 enable。
- macOS 暫時提供 CLI 與明確未實作錯誤，尚無 launchd backend。
- 新增 docs/platforms.md，記錄支援範圍及差異。

## 驗證與限制
- 本次環境無 Rust 工具鏈，且無法從容器連 GitHub，因此未執行 cargo check/test。
- 使用本地 systemd-analyze 257 verify 檢查代表性 ExecStart、Environment、restart/stop unit 語法；非 Rust 產出實測，不能視為整合測試通過。
- Linux 使用者服務先行，系統級服務、更多選項及 macOS 尚待實作。Windows 仍須回歸測試。

## 下一步
- 建立/確認三平台 CI 編譯與測試；修正實際編譯問題。
- Linux 實機驗證 user manager、參數保真、重複註冊、重啟與停止；再評估 system scope。
- 實作 launchd，補齊平台差異、文件與驗收。
- TOML、.env、統一 CRUD CLI 與 mise 專屬整合不在本期範圍。
