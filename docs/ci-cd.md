# GitHub Actions CI/CD

## 구현 상태

Tauri 2와 pnpm을 기준으로 PR 검증, 3개 플랫폼 빌드, 태그 Release를 구성했다.
Windows는 기존 named pipe와 application 기능을 유지한다. macOS에는 같은 메시지
프로토콜의 사용자 전용 Unix socket, zsh 기본 shell, CLI 실행 경로와 profile 경로를 추가했다.
Android에는 desktop daemon/tray 의존성이 없는 mobile entrypoint를 두었다.

Android 앱에서 PC 초대 링크를 입력하면 PC가 제공하는 기존 모바일 client로 이동한다.
따라서 HTTP와 WebSocket이 같은 PC origin을 사용하고 기존 승인/토큰 정책을 유지한다.
원격 페이지에 Tauri native command 권한을 부여하지 않는다. 직접 LAN/Tailscale HTTP
연결을 위해 Android manifest에서 cleartext traffic을 허용한다. Android는 PC 원격
터미널 클라이언트이며 휴대폰 안에서 desktop PTY daemon을 실행하지 않는다.

Android 생성 프로젝트는 현재 없다. runner에서 없을 때만 공식 CLI로 초기화하며,
나중에 생성 프로젝트를 커밋하면 다시 초기화하지 않는다.
플랫폼 빌드와 테스트 결과는 GitHub Actions에서 확인한다. 실제 설치/기기 동작과
인증서를 사용한 서명·공증은 별도 검증 항목이다.

## 프로젝트 분석

| 항목 | 확인 결과 |
| --- | --- |
| Tauri | v2, config schema `https://schema.tauri.app/config/2` |
| Rust Tauri / tauri-build | lockfile 기준 2.11.2 / 2.6.2 |
| JS API / CLI | lockfile 기준 API 2.11.0 / CLI 2.11.2 |
| 패키지 관리자 | pnpm 8.15.0, 기존 v6 lockfile 유지 |
| Node | CI에서 22.x 사용, package.json에 최소 22.14.0 지정 |
| Rust | edition 2021, stable + rustfmt + clippy |
| Cargo | `src-tauri/Cargo.toml`, `Cargo.lock` 커밋됨 |
| bin / lib | `rhyme-terminal`, `winmuxd`, `winmuxctl` / `winmux_lib` |
| 기존 script | dev, build, build:mobile, build:portable, preview, tauri, test |
| 추가 script | lint (ESLint), typecheck (vue-tsc), build:android |
| 테스트 | Vitest와 Rust 단위/통합 테스트가 이미 있음 |
| Android 초기화 | 없음; `gen/schemas`는 Android 프로젝트가 아님 |
| 기존 Actions | 없음 |
| bundle 설정 | 원본 `active: false` 유지, `tauri.ci.conf.json`으로 CI에서만 활성화 |

`pnpm build`는 `build:mobile → vue-tsc --noEmit → vite build` 순서다.
`build:mobile` 산출물은 `dist-mobile`, desktop 산출물은 `dist`다.
Rust build script는 `dist-mobile`을 daemon에 임베드하므로 Cargo 테스트 전에 모바일
assets를 만든다. Android Rust/Gradle 빌드와 `build:mobile`은 서로 다른 작업이다.

## Workflow 구조

| 진입점 | 실행 작업 |
| --- | --- |
| main 대상 PR | `ci.yml`: frontend 검증 + Windows 검증/설치 프로그램 빌드 병렬 |
| main push | `build.yml`: Windows / macOS / Android 재사용 workflow 병렬 호출 |
| 수동 | `build.yml`: all / windows / macos / android 중 하나 선택 |
| v* tag push | `release.yml`: 버전 일치 검사 → 3개 플랫폼 병렬 → 전체 성공 후 Release |

플랫폼 workflow 3개는 `workflow_call`만 받는다. 각각 push trigger를 두지 않아
빌드가 중복 실행되지 않는다. main과 PR push는 서로 다른 이벤트이며, tag는 Release만 실행한다.
PR/main의 같은 실행 그룹은 이전 실행을 취소한다. 수동 실행은 플랫폼별 그룹을 분리한다.
Release는 같은 tag 실행을 직렬화하며 업로드 중 취소하지 않는다.

### PR 검증

Ubuntu frontend job:

1. 고정 pnpm 설치, Node 22, pnpm store cache 복원
2. `pnpm install --frozen-lockfile`
3. `pnpm lint`
4. `pnpm typecheck`
5. `node --test scripts/ci/pipeline.test.mjs` (CI helper 회귀 검사)
6. `pnpm test` (thread pool, worker 2개, 테스트당 30초 제한)
7. `pnpm build`

Windows job:

1. 동일 frontend 환경 + Rust stable + Cargo cache
2. MSVC C++ toolchain 확인
3. `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
4. `pnpm build:mobile`
5. `cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets`
6. `cargo test --manifest-path src-tauri/Cargo.toml --locked --all-targets -- --test-threads=2`
7. `cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets`
8. 실제 release native build + NSIS EXE + MSI 생성
9. 두 설치 프로그램 존재 검사 후 artifact 업로드

Clippy의 기존 스타일 경고는 표시하지만 `-D warnings`는 사용하지 않는다.
실제 Clippy 오류는 job을 실패시킨다. 테스트 전체를 skip하거나 `continue-on-error`로
가리는 설정은 없다. 설치된 Codex가 필요한 기존 ignored test는 기본 Rust 테스트에서 제외된다.

ESLint는 JS/TypeScript recommended와 Vue essential 규칙을 적용한다.
TypeScript strict가 담당하는 unused 검사와 기존 any 타입은 lint에서 중복 강제하지 않는다.
기존 공유 reactive prop 하위 필드 갱신, Terminal 이름, Vue 타입 선언 및 URL setter 패턴은
설정에서 명시적으로 허용한다. 이 작업 때문에 앱 동작이나 이름을 바꾸지 않는다.

Branch protection에서 `Frontend checks`와 `Windows checks and installers / build`
체크를 필수로 지정하면 Windows 실패 PR의 병합을 막을 수 있다.
첫 PR 실행에서 실제 표시되는 check 이름을 확인하고 설정한다. Workflow 추가 자체가
저장소 branch protection 설정을 바꾸지는 않는다.

## 플랫폼별 빌드

### Windows — 최우선

Runner: `windows-latest` (x64 MSVC).

```powershell
pnpm tauri build --ci --config src-tauri/tauri.ci.conf.json --bundles nsis,msi -- --locked
```

GitHub hosted Windows image의 MSVC, Windows SDK, WebView2를 사용한다.
Tauri bundler가 필요한 WiX/NSIS 도구를 내려받는다.
Tauri의 `beforeBuildCommand`가 frontend 전체 빌드를 실행하므로 별도로 중복 실행하지 않는다.

- EXE: `src-tauri/target/release/bundle/nsis/*.exe`
- MSI: `src-tauri/target/release/bundle/msi/*.msi`
- Artifact: `windows-x64-installers`

EXE는 NSIS 설치 프로그램이다. 기존 portable 빌드는 `pnpm build:portable`로 유지한다.
설치된 앱은 같은 GUI 실행 파일의 `--winmux-daemon` 경로를 사용할 수 있으므로
별도 daemon sidecar 경로를 임의로 추가하지 않았다.

### macOS

Runner: `macos-latest`. 해당 hosted runner의 기본 CPU architecture로 빌드한다.
Universal binary나 Intel/Apple Silicon 양쪽 지원을 검증한 것으로 간주하지 않는다.

```sh
pnpm tauri build --ci --config src-tauri/tauri.ci.conf.json --bundles dmg -- --locked
```

- DMG: `src-tauri/target/release/bundle/dmg/*.dmg`
- Artifact: `macos-native-dmg`
- 인증서가 없으면 `APPLE_SIGNING_IDENTITY=-`로 ad-hoc 서명한다.
  Apple Developer 인증서/공증은 필요하지 않다.
- DMG에 포함된 .app을 별도 zip으로 중복 업로드하지 않는다.
- Unix socket IPC를 사용하고 zsh를 기본 shell로 선택한다.

### Android

Runner: `ubuntu-latest`. JDK 17 (Temurin), Android SDK 36, build-tools 36.0.0,
NDK 27.2.12479018, Rust `aarch64-linux-android`를 사용한다.

Android 폴더가 없는 경우에만:

```sh
pnpm tauri android init --ci --skip-targets-install
```

main/수동 (개발용 debug APK + AAB):

```sh
pnpm tauri android build --ci --target aarch64 --apk --aab --debug
```

tag (release APK + AAB):

```sh
pnpm tauri android build --ci --target aarch64 --apk --aab
```

Gradle 출력은 `src-tauri/gen/android/app/build/outputs` 아래에서 실제 variant별로 찾는다.
`collect-android.mjs`는 APK/AAB 각각 한 개가 존재하는지 검사하고
`artifacts/android/rhyme-terminal-android-arm64-{debug,unsigned,release}.{apk,aab}`로 복사한다.
파일이 없거나 ABI 설정 변경으로 여러 개가 나오면 실패하므로 업로드 누락을 숨기지 않는다.

- `debug`: Android SDK 기본 debug key로 서명됨. Secret 없이 설치 가능한 개발 APK.
- `unsigned`: tag release이지만 키가 없음. 설치/스토어 제출 전에 별도 서명 필요.
- `release`: 등록한 keystore로 서명됨.
- AAB는 기기에 직접 설치하는 파일이 아니며 Play/bundletool용이다.
- runner의 debug key는 매 실행마다 달라질 수 있어 이전 APK 위에 업데이트 설치가
  안 되면 기존 개발 앱 제거가 필요할 수 있다.
- `ANDROID_ABI`, Rust target, artifact collector를 함께 확장하면 다른 ABI를 추가할 수 있다.
- Android는 `tauri.android.conf.json`을 자동 병합하고 `pnpm build:android`로 전용 화면을 빌드한다.

Android CLI는 Cargo 직접 빌드 옵션과 Gradle runner 옵션이 다르므로 desktop의
`-- --locked`를 Android 명령에 임의로 붙이지 않는다.

## 수동 빌드

1. 저장소의 **Actions → Build → Run workflow**를 연다.
2. branch를 선택한다.
3. `platform`에서 `windows`, `macos`, `android`, `all` 중 하나를 선택한다.
4. **Run workflow**를 누른다.

모바일 브라우저에서도 입력 하나만 선택하면 된다. GitHub 앱에 수동 실행 메뉴가 보이지 않으면
해당 Actions 페이지를 모바일 브라우저로 연다. Workflow 파일이 default branch에 올라와야
수동 실행 메뉴가 표시된다.

## Release 생성

먼저 세 플랫폼을 수동으로 성공시킨 후 tag를 만든다. package.json, src-tauri/Cargo.toml,
src-tauri/tauri.conf.json 버전을 같은 값으로 맞추고 Cargo.lock 변경도 반영한다.

```sh
git tag v0.1.0
git push origin v0.1.0
```

`v*` tag 이벤트가 시작점이다. 버전 검사에서 tag와 앱 버전이 다르면 빌드 전에 실패한다.
플랫폼 3개가 모두 성공하면 GitHub CLI가 `GITHUB_TOKEN`으로 draft Release를 생성하고
EXE/MSI/DMG/APK/AAB 모두 업로드한 다음 공개한다. PAT는 필요 없다.
재실행은 같은 Release를 재사용하고 동일한 파일명을 교체한다.

플랫폼 빌드가 하나라도 실패하면 Release를 공개하지 않는다. 성공한 플랫폼 artifact는 별도로 다운로드할 수 있다.

## Artifact 위치와 캐시

Actions → 해당 실행 → Artifacts:

- `windows-x64-installers`
- `macos-native-dmg`
- `android-arm64-packages`

각 artifact는 14일 보관한다. GitHub Release 첨부 파일은 Actions artifact 보관 기간과 별개다.

- frontend: setup-node가 OS별 pnpm store를 lockfile 기반으로 캐시한다.
- Rust: Swatinem/rust-cache가 Cargo registry/git/target을 캐시한다.
  OS/architecture/toolchain/lockfile과 플랫폼별 shared-key를 사용한다.
  PR은 Cargo 캐시 저장을 하지 않는다.
- Android: OS/architecture와 NDK, lockfile, Gradle 설정 기반으로 Gradle cache를 분리한다.
- 빌드 파일 전체나 signing keystore를 artifact/cache에 올리지 않는다.

## Secrets와 서명

기본 Windows 설치 프로그램과 macOS ad-hoc, Android 개발 APK에는 사용자 Secret이 필요 없다.
Release 업로드는 GitHub이 제공하는 `GITHUB_TOKEN`을 쓴다.

Settings → Secrets and variables → Actions → Repository secrets에서 필요할 때 등록한다.
현재 signing secrets는 **tag Release에서만** 명시적으로 재사용 workflow에 전달한다.
main/수동 빌드와 PR에는 전달하지 않는다.

### Apple signing / notarization

| Secret | 용도 |
| --- | --- |
| APPLE_CERTIFICATE | Developer ID Application 인증서 .p12의 base64 |
| APPLE_CERTIFICATE_PASSWORD | .p12 암호 |
| APPLE_SIGNING_IDENTITY | `Developer ID Application: 이름 (TEAMID)` |
| APPLE_ID | 공증용 Apple 계정 |
| APPLE_PASSWORD | 앱 전용 암호 |
| APPLE_TEAM_ID | Apple Developer Team ID |

서명에는 앞의 3개를 함께 설정한다. 공증을 활성화하려면 뒤의 3개도 함께 설정한다.
runner 임시 keychain에 인증서를 import하고 Tauri가 codesign/notarytool을 호출한다.
`always()` cleanup으로 임시 keychain과 p12를 제거한다. 인증서 없이 빌드할 때는
ad-hoc 서명이며 Gatekeeper가 신뢰하는 공증 배포와는 다르다.

[공식 macOS 서명 문서](https://v2.tauri.app/distribute/sign/macos/)

### Android release signing

로컬에서 업로드 keystore를 만든 후 파일을 공개 저장소에 넣지 않고 base64만 Secret에 저장한다.

```sh
keytool -genkeypair -v -keystore upload-keystore.jks -storetype JKS -keyalg RSA -keysize 2048 -validity 10000 -alias upload
```

| Secret | 용도 |
| --- | --- |
| ANDROID_KEYSTORE_BASE64 | keystore의 base64 |
| ANDROID_KEY_ALIAS | 키 alias (예: upload) |
| ANDROID_KEY_PASSWORD | 키 암호 |
| ANDROID_STORE_PASSWORD | keystore 암호 |

`android-signing.mjs`가 release 때만 임시 keystore를 만들고 생성 Gradle 파일에
환경변수 기반 signingConfig를 연결한다. 암호를 YAML 명령문이나 Gradle 소스에 삽입하지 않는다.
생성 프로젝트를 나중에 커밋해도 같은 스크립트를 사용하며, 기존 Gradle의 별도 서명 체계와
중복되지 않도록 확인한다. Secret 일부만 등록하면 명확한 오류로 실패한다.
keystore가 없으면 release는 unsigned, main/수동은 debug를 유지한다.

[공식 Android 서명 문서](https://v2.tauri.app/distribute/sign/android/)

## Public repository 보안

- `pull_request` 사용, `pull_request_target` 사용 안 함.
- PR 코드가 실행되는 job은 `contents: read`, signing/deploy Secret 없음.
- checkout의 `persist-credentials: false`.
- Release publish job에만 `contents: write`. 이 job은 source를 checkout하거나 실행하지 않는다.
- 서명 Secret은 필요한 플랫폼에 이름별로 전달하고 `secrets: inherit`를 사용하지 않는다.
- GitHub 공식 Action, pnpm, dtolnay, Swatinem, android-actions만 사용한다.
- 외부 Action은 조회한 commit SHA로 고정하고 Dependabot이 월별 업데이트를 제안한다.
- tag/branch protection과 Actions 사용 권한 설정은 저장소 관리자가 추가로 관리한다.

## 검증 방법

Windows PowerShell에서는 환경에 따라 `pnpm.cmd`를 사용한다.

```powershell
pnpm.cmd install --frozen-lockfile
pnpm.cmd lint
pnpm.cmd typecheck
pnpm.cmd test
node --test scripts/ci/pipeline.test.mjs
pnpm.cmd build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets
cargo test --manifest-path src-tauri/Cargo.toml --locked --all-targets -- --test-threads=2
cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets
pnpm.cmd tauri build --ci --config src-tauri/tauri.ci.conf.json --bundles nsis,msi -- --locked
git diff --check
```

workflow 표현식까지 검사하려면 `actionlint`를 저장소 루트에서 실행한다.
macOS DMG/Android APK는 각 runner에서 빌드한 뒤 실제 기기에서 설치/실행을 확인해야 한다.
Windows에서 CLI help를 확인하는 것만으로 macOS/Android 빌드 성공을 증명할 수 없다.

## 흔한 실패와 해결

| 실패 | 원인과 대응 |
| --- | --- |
| macOS의 Windows API import 오류 | 새 코드의 `cfg(windows)` 또는 transport 분리를 확인 |
| Android tray/native TLS/desktop API 오류 | desktop 전용 의존성이 common dependencies에 추가되었는지 확인 |
| APK의 PC 연결 실패 | PC 모바일 연결 활성화, 같은 LAN/Tailscale, 초대 만료, PC 승인, 방화벽 확인 |
| 설치 파일이 없음 | CI config overlay와 bundles 옵션 확인. 기본 config는 portable용 bundle 비활성화 |
| mobile asset 테스트 실패 | Cargo 전에 `pnpm build:mobile` 실행 |
| `ERR_PNPM_OUTDATED_LOCKFILE` | 같은 pnpm 8.15.0으로 lockfile 갱신 후 커밋 |
| Node engine 오류 | Node 22 사용. 새 ESLint/Vitest는 오래된 Node에서 동작하지 않음 |
| Rust fmt 실패 | `cargo fmt --manifest-path src-tauri/Cargo.toml --all` 실행 후 diff 검토 |
| Cargo build lock | 같은 target을 쓰는 다른 cargo 작업이 끝난 뒤 재시도 |
| Windows WiX/NSIS 다운로드 실패 | runner 네트워크/방화벽/업스트림 배포 상태 확인 후 재실행 |
| `JAVA_HOME` / NDK 오류 | JDK 17 및 `NDK_HOME=$ANDROID_HOME/ndk/27.2.12479018` 확인 |
| Android 폴더는 있으나 Gradle 없음 | 불완전한 초기화. CI는 기존 폴더를 덮어쓰지 않고 실패 |
| `Resource not accessible by integration` | publish의 contents: write 및 저장소 Actions 정책 확인 |
| Release가 실행되지 않음 | `v*` tag를 remote로 push했는지 확인 |
| Release publish skipped | 필요한 세 플랫폼 중 실패 job을 먼저 해결 |
| 부분 signing secret | 해당 플랫폼의 필수 Secret을 모두 등록하거나 전부 제거 |
| Windows 로컬 pnpm EPERM/모듈 접근 오류 | dependency 링크/권한/다른 install 프로세스 확인 후 단독 재실행 |


## 검증 기록

초기 Windows CI 구성은 로컬에서 frontend/Rust 테스트 및 NSIS/MSI 빌드를 통과했다.
이후 macOS/Android 대응 변경은 사용자의 요청에 따라 GitHub hosted runner에서 검증한다.
Actions → PR CI → Run workflow로 PR과 같은 frontend/Windows 검증을 수동 실행할 수 있다.
Actions → Build → all로 세 플랫폼의 artifact 생성을 확인한다.

## 변경 파일

- `.github/workflows/ci.yml`, `build.yml`, `build-windows.yml`, `build-macos.yml`,
  `build-android.yml`, `release.yml`: 이벤트별 검증/빌드/Release.
- `.github/actions/setup-frontend/action.yml`: 공통 Node/pnpm 설치와 cache.
- `.github/dependabot.yml`: 고정 Action SHA 업데이트.
- `scripts/ci/apple-keychain.sh`, `android-signing.mjs`, `collect-android.mjs`,
  `check-release-version.mjs`, `pipeline.test.mjs`: 서명 준비, 산출물 검사, 버전 검사와 회귀 테스트.
- `src-tauri/tauri.ci.conf.json`: 원래 portable 설정을 유지하는 CI bundle overlay.
- `package.json`, `pnpm-lock.yaml`, `eslint.config.js`, `vitest.config.ts`:
  실제 lint/typecheck 명령과 개발 도구, Windows에서도 안정적인 테스트 실행 설정.
- `.gitattributes`, `.gitignore`: shell script LF 유지와 signing/빌드 파일 제외.
- `src-tauri/src/commands.rs`, `src-tauri/src/bin/winmuxctl.rs`: rustfmt 정리만 적용.
- `src/components/PalettePopover.vue`: 곧바로 덮어쓰는 불필요한 초기 대입만 제거.
- `src-tauri/src/ipc/transport.rs`, `platform.rs`, `unix_cli.rs` 및 desktop 모듈: macOS IPC와 CLI 실행 환경.
- `src-tauri/src/mobile.rs`, `tauri.android.conf.json`, `src/native-mobile/`, `native-mobile.html`, `vite.android.config.ts`: Android 진입점과 PC 연결 화면.
- `src/mobile/App.vue`, `src/lib/terminal-config.ts`, `terminal-launch.ts`, `flow-types.ts`: native client 복귀 및 macOS 기본 shell/명령.
- `scripts/ci/prepare-android.mjs`: LAN/Tailscale 연결 manifest 설정.
- `docs/ci-cd.md`: 이 문서.

기존 ExplorerPanel/SideBar/한국어 locale 및 다른 미커밋 작업은 이 변경의 일부가 아니다.

## 향후 개선

macOS zsh CWD 추적, 플랫폼별 설치 후 실제 기기 smoke test, macOS universal 또는 architecture matrix, Android ABI matrix와 Play App Signing,
Windows 코드 서명, 설치 후 smoke test, branch protection 필수 체크를 추가한다.
캐시와 빌드 시간을 실제 Actions 기록으로 측정한 뒤 병렬화나 debug/release 재사용을 조정한다.

참고: [Tauri 2 GitHub pipeline](https://v2.tauri.app/distribute/pipelines/github/),
[플랫폼별 prerequisites](https://v2.tauri.app/start/prerequisites/).
