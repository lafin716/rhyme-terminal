# winmux

Rhyme Flow: [로컬 자동화 사용법과 검증 안내](docs/rhyme-flow.md)

Windows에서 여러 터미널 세션을 한 창 안에 나눠 쓰기 위한 터미널 멀티플렉서입니다.  
Tauri v2 + Vue 3 + xterm.js로 만들어졌고, 백그라운드 daemon(`winmuxd`)이 PTY 세션을 관리합니다.

> tmux처럼 빠르게 터미널을 만들고, pane을 나누고, 탭을 옮기고, 작업 공간별로 터미널 환경을 분리하는 데 초점을 둔 Windows 데스크톱 앱입니다.

---

## 주요 기능

- **여러 터미널 세션 관리**
  - PowerShell, PowerShell 7, Command Prompt, WSL, Git Bash, custom shell 지원
  - 세션 탭 rename, 닫기, 휠클릭 닫기, 우클릭 메뉴 지원
- **Pane 분할**
  - 가로/세로 split
  - 방향키 기반 split-or-move
  - 4분할 quadrant split
  - 최대 16개 pane 제한
- **Workspace**
  - 작업 공간별 layout, 기본 폴더, 터미널 preset 설정
  - 사이드바 compact / expanded / minimal 모드
- **드래그 앤 드롭 탭 이동**
  - 탭 순서 변경
  - 다른 pane으로 이동
  - pane edge에 drop해서 split 생성
- **파일/URL 링크 열기**
  - 터미널 출력의 URL Ctrl+클릭으로 내장 브라우저 열기
  - `ls`, `ll`, 빌드 로그 등에 나온 파일 경로 Ctrl+클릭으로 파일 미리보기
  - 텍스트 파일은 Monaco 기반 viewer, 이미지는 image preview
- **Command Palette**
  - 터미널에서 마우스 middle-click으로 palette 열기
  - 자주 쓰는 명령을 paste 또는 auto-run
  - context menu / radial menu 표시 방식 선택
- **tmux 스타일 prefix key**
  - `Ctrl+B` 후 키 입력으로 세션/분할/rename 등 실행
  - 일부 단축키는 Settings에서 변경 가능
- **System tray**
  - 창 닫기 시 앱 종료가 아니라 tray로 숨김

---

## 스크린샷

아직 실제 스크린샷 파일은 포함하지 않았습니다. 아래 영역에 직접 이미지를 넣으면 됩니다.

### 메인 화면

> **이미지 넣을 위치**  
> 추천 파일명: `docs/images/main-window.png`  
> 촬영 설명: 왼쪽 workspace 사이드바, 상단 메뉴, 여러 터미널 탭, 하단 status bar가 보이도록 캡처하세요.

<!--
예시:
![winmux main window](docs/images/main-window.png)
-->

### Pane 분할과 탭 이동

> **이미지 넣을 위치**  
> 추천 파일명: `docs/images/split-panes.png`  
> 촬영 설명: 2~4개 pane으로 분할된 화면과 각 pane의 탭을 함께 보여주세요.

<!--
![split panes](docs/images/split-panes.png)
-->

### Settings

> **이미지 넣을 위치**  
> 추천 파일명: `docs/images/settings.png`  
> 촬영 설명: Terminal preset, Workspace default folder, Keybindings, Palette 설정 중 하나를 보여주세요.

<!--
![settings](docs/images/settings.png)
-->

### 파일 미리보기 / 브라우저 탭

> **이미지 넣을 위치**  
> 추천 파일명: `docs/images/resource-tabs.png`  
> 촬영 설명: 터미널에서 파일 또는 URL을 열어 file viewer/browser tab이 같이 보이는 장면을 캡처하세요.

<!--
![resource tabs](docs/images/resource-tabs.png)
-->

---

## 빠른 시작

### 1. 사전 준비

Windows 환경을 기준으로 합니다.

- Windows 10/11
- [Rust](https://www.rust-lang.org/tools/install)
- [Node.js](https://nodejs.org/)
- [pnpm](https://pnpm.io/)
- Microsoft Edge WebView2 Runtime
  - 최신 Windows 10/11에는 보통 기본 설치되어 있습니다.

pnpm이 없다면:

```powershell
npm install -g pnpm
```

### 2. 의존성 설치

```powershell
pnpm install
```

### 3. 개발 모드 실행

```powershell
pnpm tauri dev
```

Vite dev server는 `http://localhost:3000`에서 실행되고, Tauri 앱이 이 dev server를 사용합니다.

### 4. 프론트엔드 빌드 확인

```powershell
pnpm build
```

### 5. Portable Windows exe 빌드

```powershell
pnpm build:portable
```

생성 위치:

```text
src-tauri/target/release/winmux.exe
```

이 빌드는 installer 없이 실행할 수 있는 portable GUI exe를 만듭니다.

---

## 기본 사용법

### 새 터미널 만들기

- `+` 버튼 클릭
- 또는 단축키:

```text
Ctrl+N
```

`+` 옆의 드롭다운 버튼을 누르면 PowerShell, PowerShell 7, Command Prompt, WSL, Git Bash 등 다른 터미널 preset으로 새 세션을 만들 수 있습니다.

### 터미널 탭 조작

| 동작 | 방법 |
|---|---|
| 탭 선택 | 탭 클릭 |
| 탭 이름 변경 | 탭 더블클릭 또는 우클릭 → Rename |
| 현재 탭 닫기 | 닫기 버튼, 우클릭 → Close Current Tab, 또는 휠클릭 |
| 다른 탭 닫기 | 우클릭 → Close Other Tabs |
| 모든 탭 닫기 | 우클릭 → Close All Tabs |
| 탭 순서 변경 | 탭을 좌우로 드래그 |
| 다른 pane으로 이동 | 탭을 다른 pane 중앙으로 드래그 |
| split하면서 이동 | 탭을 다른 pane의 위/아래/왼쪽/오른쪽 edge로 드래그 |

터미널 세션을 닫을 때는 확인창이 뜹니다. 확인창에서 “다시 묻지 않기”를 선택하면 이후에는 바로 닫힙니다.

### Pane 분할

기본 prefix key는 `Ctrl+B`입니다.

```text
Ctrl+B %
```

가로 방향으로 pane을 나눕니다.

```text
Ctrl+B "
```

세로 방향으로 pane을 나눕니다.

또는 `Ctrl+Alt+방향키`를 사용할 수 있습니다.

- 해당 방향에 이웃 pane이 있으면 현재 탭을 그 pane으로 이동
- 이웃 pane이 없으면 해당 방향으로 새 pane 생성

예:

```text
Ctrl+Alt+Right
```

오른쪽 pane이 있으면 현재 탭을 오른쪽 pane으로 이동하고, 없으면 오른쪽에 새 pane을 만듭니다.

### Workspace 사용

왼쪽 사이드바에서 workspace를 전환합니다.

- workspace 클릭: 전환
- workspace 더블클릭: 이름 변경
- workspace 우클릭: rename/delete 메뉴
- `+` 버튼: 새 workspace 생성
- `Ctrl+Shift+B`: 사이드바 표시 모드 순환

Settings에서 workspace별 기본 폴더를 지정할 수 있습니다.  
예를 들어 `frontend`, `backend`, `infra` workspace를 만들고 각각 다른 default folder를 지정하면 새 터미널이 해당 위치에서 시작합니다.

### 파일 열기 예시

터미널에서 파일 목록이나 빌드 로그에 나온 파일 경로를 Ctrl+클릭하면 파일 미리보기가 열립니다.

예:

```powershell
ls
```

출력된 `package.json`, `src/App.vue` 같은 파일명에 마우스를 올리면 링크처럼 표시되고, Ctrl+클릭하면 viewer tab으로 열립니다.

빌드 에러 로그 예:

```text
src/components/Terminal.vue:120:15
```

이런 형식도 파일 경로와 line/column 정보로 처리됩니다.

### URL 열기 예시

터미널 출력의 URL을 Ctrl+클릭하면 내장 browser tab으로 열립니다.

```powershell
echo https://github.com
```

### Palette 사용 예시

터미널 영역에서 마우스 middle-click을 누르면 command palette가 열립니다.

예를 들어 Settings → Palette에서 아래 항목을 추가할 수 있습니다.

| Label | Command | Auto-run |
|---|---|---|
| Git status | `git status` | 켬 |
| PNPM build | `pnpm build` | 켬 |
| Clear | `clear` | 켬 |
| Docker ps | `docker ps` | 켬 |

Auto-run이 켜져 있으면 명령이 입력된 뒤 Enter까지 자동으로 전송됩니다.  
꺼져 있으면 프롬프트에 붙여넣기만 됩니다.

---

## 기본 단축키

| Action | 기본 단축키 | Prefix |
|---|---:|---:|
| New Terminal | `Ctrl+N` | `Ctrl+B c` |
| Close Focused Session | `Ctrl+W` | - |
| Kill Focused Session | - | `Ctrl+B &` |
| Rename Session | - | `Ctrl+B ,` |
| Next Tab in Pane | - | `Ctrl+B n` |
| Previous Tab in Pane | - | `Ctrl+B p` |
| Split Horizontally | - | `Ctrl+B %` |
| Split Vertically | - | `Ctrl+B "` |
| Hide Window | - | `Ctrl+B d` |
| Open Settings | `Ctrl+,` | - |
| Focus Previous Session | `Ctrl+Shift+{` | - |
| Focus Next Session | `Ctrl+Shift+}` | - |
| Split or Move Left | `Ctrl+Alt+Left` | - |
| Split or Move Right | `Ctrl+Alt+Right` | - |
| Split or Move Up | `Ctrl+Alt+Up` | - |
| Split or Move Down | `Ctrl+Alt+Down` | - |
| Quadrant Top-Left Split | `Ctrl+Alt+I` | - |
| Quadrant Top-Right Split | `Ctrl+Alt+O` | - |
| Quadrant Bottom-Left Split | `Ctrl+Alt+K` | - |
| Quadrant Bottom-Right Split | `Ctrl+Alt+L` | - |
| Cycle Sidebar Mode | `Ctrl+Shift+B` | - |

숫자 선택은 prefix와 함께 사용할 수 있습니다.

```text
Ctrl+B 0
Ctrl+B 1
Ctrl+B 2
...
```

현재 pane 안에서 해당 index의 탭을 선택합니다.

> 참고: Settings → Keybindings에서 일반 단축키는 변경할 수 있습니다. Prefix mapping은 현재 버전에서 고정입니다.

---

## Settings 가이드

Settings는 `Ctrl+,`로 열 수 있습니다.

### Terminal

새 터미널을 만들 때 사용할 기본 shell을 설정합니다.

지원 preset:

- Windows PowerShell: `powershell.exe`
- PowerShell 7: `pwsh.exe`
- Command Prompt: `cmd.exe`
- WSL: `wsl.exe`
- Git Bash: `C:\Program Files\Git\bin\bash.exe`
- Custom

예: Git Bash를 기본 터미널로 쓰고 싶다면:

1. Settings → Terminal
2. Global default → Preset에서 `Git Bash` 선택
3. 이후 새 터미널은 Git Bash로 시작

workspace별 override도 가능합니다.  
예를 들어 `Windows` workspace는 PowerShell, `Linux` workspace는 WSL로 시작하게 설정할 수 있습니다.

### Workspaces

workspace별 기본 폴더를 설정합니다.

예:

| Workspace | Default folder |
|---|---|
| frontend | `C:\work\my-app` |
| backend | `C:\work\my-api` |
| scripts | `C:\work\scripts` |

이렇게 설정하면 해당 workspace에서 새로 만든 터미널은 지정된 폴더에서 시작합니다.

### Keybindings

각 action의 단축키를 변경하거나 해제할 수 있습니다.

- key cell 클릭 → 새 단축키 입력
- Reset → 기본값으로 복구
- Clear → 단축키 비활성화
- Reset all → 전체 기본값 복구

충돌하는 단축키가 있으면 경고가 표시됩니다.

### Palette

middle-click palette에 표시할 명령을 관리합니다.

- Label: 메뉴에 보일 이름
- Command: 터미널에 입력할 명령
- Auto-run: Enter까지 자동 실행할지 여부
- Display style: context menu 또는 radial menu

---

## CLI와 daemon

winmux는 GUI 앱과 별도로 daemon이 PTY 세션을 관리합니다.

- GUI 앱: `winmux.exe`
- daemon: `winmuxd.exe`
- CLI: `winmuxctl.exe`

daemon은 첫 Tauri invoke 시 자동으로 실행됩니다.  
IPC는 Windows named pipe를 사용합니다.

```text
\\.\pipe\winmux-{user}
```

### CLI 빌드

CLI/daemon을 직접 빌드하려면:

```powershell
cd src-tauri
cargo build --release --bin winmuxctl --bin winmuxd
```

생성 위치:

```text
src-tauri/target/release/winmuxctl.exe
src-tauri/target/release/winmuxd.exe
```

### CLI 예시

세션 목록:

```powershell
.\src-tauri\target\release\winmuxctl.exe ls
```

새 세션 생성:

```powershell
.\src-tauri\target\release\winmuxctl.exe new --name scratch --shell powershell.exe
```

작업 폴더를 지정해서 생성:

```powershell
.\src-tauri\target\release\winmuxctl.exe new --name repo --cwd C:\work\my-repo --shell pwsh.exe
```

Git Bash 세션 생성:

```powershell
.\src-tauri\target\release\winmuxctl.exe new `
  --name bash `
  --shell "C:\Program Files\Git\bin\bash.exe" `
  --shell-arg "--login" `
  --shell-arg "-i"
```

세션 종료:

```powershell
.\src-tauri\target\release\winmuxctl.exe kill scratch
```

세션 이름 변경:

```powershell
.\src-tauri\target\release\winmuxctl.exe rename scratch server
```

daemon ping:

```powershell
.\src-tauri\target\release\winmuxctl.exe ping
```

daemon 종료:

```powershell
.\src-tauri\target\release\winmuxctl.exe kill-server
```

---

## 프로젝트 구조

```text
.
├── src/                 # Vue 3 frontend
│   ├── components/      # UI components
│   ├── composables/     # 상태/동작 로직
│   └── lib/             # keybindings, tauri bridge, persistence 등
├── src-tauri/           # Rust/Tauri backend
│   ├── src/bin/         # winmuxd, winmuxctl
│   ├── src/ipc/         # named-pipe IPC
│   ├── src/pty/         # portable-pty wrapper
│   └── src/commands.rs  # Tauri command handlers
├── public/              # static assets
├── dist/                # Vite build output
├── package.json
├── vite.config.ts
└── tsconfig.json
```

---

## 개발 명령어

Frontend dev server만 실행:

```powershell
pnpm dev
```

Tauri 앱 개발 모드:

```powershell
pnpm tauri dev
```

프론트엔드 타입체크 + Vite build:

```powershell
pnpm build
```

Portable GUI exe 빌드:

```powershell
pnpm build:portable
```

Rust backend만 빌드:

```powershell
cd src-tauri
cargo build
```

---

## 데이터 저장 위치

프론트엔드 설정은 localStorage에 저장됩니다.

주요 저장 항목:

- preferences
- workspace layout
- keybindings
- palette items

저장 key는 아래 형식을 사용합니다.

```text
winmux:{domain}:v1
```

daemon 로그:

```text
%LOCALAPPDATA%\winmux\logs\winmuxd.log
```

---

## 문제 해결

### 앱은 켜지는데 터미널이 생성되지 않습니다

1. Settings → Terminal에서 Program 경로가 올바른지 확인하세요.
2. Git Bash를 사용하는 경우 기본 경로는 보통 아래입니다.

```text
C:\Program Files\Git\bin\bash.exe
```

3. daemon 로그를 확인하세요.

```text
%LOCALAPPDATA%\winmux\logs\winmuxd.log
```

### Git Bash가 바로 종료됩니다

Arguments가 아래처럼 설정되어 있는지 확인하세요.

```text
--login
-i
```

또는 preset을 `Git Bash`로 다시 선택해 기본값을 복구하세요.

### WSL이 열리지 않습니다

PowerShell에서 먼저 WSL이 정상 동작하는지 확인하세요.

```powershell
wsl.exe --status
wsl.exe
```

WSL 배포판이 설치되어 있지 않으면 Microsoft Store 또는 `wsl --install`로 설치해야 합니다.

### Ctrl+클릭으로 파일이 열리지 않습니다

파일 경로가 현재 터미널의 working directory 기준으로 존재해야 합니다.  
workspace default folder나 shell의 현재 위치를 확인하세요.

예:

```powershell
pwd
ls
```

PowerShell/Git Bash/WSL preset은 prompt hook을 통해 현재 경로를 winmux에 알려주도록 구성되어 있습니다.

### 빌드 시 chunk size warning이 보입니다

Vite가 Monaco editor 등 큰 chunk에 대해 경고할 수 있습니다.

```text
Some chunks are larger than 500 kB after minification
```

현재는 빌드 실패가 아니라 경고입니다.

### WebView2 관련 오류가 납니다

Microsoft Edge WebView2 Runtime이 설치되어 있는지 확인하세요.  
최신 Windows 10/11은 대부분 기본 포함되어 있지만, 오래된 환경에서는 별도 설치가 필요할 수 있습니다.

---

## 설계 메모

- backend mutex는 `parking_lot::Mutex`를 사용합니다.
- PTY 출력은 Tauri event에서 base64로 전달됩니다.
- daemon IPC는 Windows named pipe + length-prefixed frame 기반입니다.
- 세션 이름은 workspace index prefix를 포함합니다.

```text
w{workspaceIndex}.{name}
```

예:

```text
w0.session-1
w1.server
```

UI에서는 prefix를 제거한 이름만 보여줍니다.

---

## 상태

현재 버전은 `0.1.0`입니다.  
아직 테스트 스위트와 CI는 구성되어 있지 않습니다.

