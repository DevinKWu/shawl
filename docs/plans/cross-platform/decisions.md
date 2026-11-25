# 決策紀錄
- 保留公開 Fork 與 master；新增 develop、feature/cross-platform。
- 採用平台原生服務管理器而非自製跨平台常駐 daemon，降低維運負擔；平台語意差異由 adapter 明確處理。
- TOML 為預定共用格式，mise 為選用執行環境，不強制依賴 PowerShell 或 Node.js。
- 保留原有 Windows CLI 與 MIT 授權。

- 更新決策：取消 mise 專屬整合，取代前述選用執行環境規劃。僅提供通用 executable/args/working directory/env，不限制外部工具。

- 2026-11-26：縮減跨平台範圍，取代原共用 ServiceSpec/ServiceManager、TOML、dotenv 與統一管理 CLI 的本期規劃。保留 Windows 實作，僅新增 Linux/systemd 與 macOS/launchd；必要時才抽離共用邏輯。設定檔及管理 CLI 延後獨立開發，降低回歸與上游同步成本。

- Linux 初版採 systemd 使用者服務，不預設 root、不直接啟動或 enable；原生管理器負責重啟、停止與 cgroup 清理。先驗證有限選項，再擴充 system scope 與 macOS。拒絕未支援選項，環境變數不承諾秘密保護。
