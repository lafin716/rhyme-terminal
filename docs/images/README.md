# README 스크린샷

실제 Rhyme Terminal 프런트엔드를 1600 × 960 크기의 격리된 Chromium에서 렌더링해 캡처합니다. 이미지 합성이나 UI 스타일 수정은 하지 않습니다.

| 이미지 | 내용 |
| --- | --- |
| `terminal-workspace.png` | 프로젝트 탐색, 세 개의 터미널, AI 제공자 프로필 태그, 파일 탐색기 |
| `rhyme-flow.png` | 명령·승인·출력 노드로 구성한 예제 Flow |
| `ai-providers.png` | Claude Code·Codex의 예제 프로필과 기본 계정 설정 |

프로젝트·프로필·Flow 정의·터미널 출력은 촬영 전용 예제입니다. Tauri 호출을 모의 응답으로 대체하므로 실제 계정, 사용자 파일, PTY, AI 제공자 또는 실행 기록에 접근하지 않습니다. 데모 터미널의 텍스트는 실제 CLI 실행 결과가 아니며, Flow도 실행하지 않습니다. 네이티브 Tauri 창이나 휴대폰 하드웨어를 검증한 자료는 아닙니다.

## 다시 촬영하기

Windows, Node.js 22 이상, Chrome이 필요합니다. 저장소 루트에서 실행합니다.

```powershell
pnpm.cmd build
node scripts/capture-readme.mjs
```

Chrome이 기본 경로에 없으면 실행 파일 위치를 지정합니다.

```powershell
$env:CHROME_PATH = 'C:\path\to\chrome.exe'
node scripts/capture-readme.mjs
```

[촬영 스크립트](../../scripts/capture-readme.mjs)는 예제 데이터를 주입하고 UI를 직접 조작합니다. 화면 캡처 전 터미널 개수, Flow 노드, AI 제공자 제목과 상단 컨트롤을 확인합니다. 촬영 후 브라우저와 임시 HTTP 서버를 종료하며, 디버깅용 격리 프로필은 `.scratch/readme-capture/chrome-*`에 남깁니다.
