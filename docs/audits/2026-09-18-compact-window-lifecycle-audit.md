# Port Lens — Compact 창 수명주기 재검수 및 후속 작업 명세

작성일: 2026-09-18 (Asia/Seoul)  
대상: `teeeeooo/port-lens` / Draft PR #5  
상태: **로컬 수정 구현·회귀 26/26 PASS. 커밋·push 요청이 도구에서 차단되어 원격 미반영. Windows 실사용 미검증, 릴리스 교체 아님.**

수정 위치: `/Users/sunjaekim/Developer/port-lens-audit-20260918`의 기존 `fix/product-audit-2026-09-18` 작업트리. 부모 HEAD는 `363e77b`이고 이번 변경의 새 커밋 SHA는 없다. 원격 Draft PR #5에는 이 후속 변경이 아직 올라가지 않았다.

## 현재 릴리스 범위 — 2026-09-18 사용자 결정

아래 감사와 미커밋/원격 미반영 표시는 이전 인계 시점의 기록이다. 현재 목표는 기존 purpose-audit + CL-01~CL-10 변경으로 Windows 드래그를 검증하고, 이상이 없으면 릴리스하는 것이다. **N01~N03, CL-11~CL-12는 당분간 열지 않으며 이번 릴리스의 선행조건이 아니다.** 아래 후속 명세는 보류 자료로만 남긴다.

검증은 후보 SHA의 Windows/macOS CI와 Windows 패키지 빌드, 그리고 실제 Windows에서 hover 닫힘/열림, 갱신 시점, 긴 드래그·연속 재시작, 취소/Alt-Tab, 종료 후 갱신, 일반 Open/트레이 Open을 확인한다. 기존에 수용한 짧은 시작 hitch는 동일 기준으로 판단하고, 악화·지속 커서 offset·catch-up jump·hover/갱신/Open 고착이 없는지를 본다. 문서의 3개 비교군 각100회 계측, native API 실패 주입, 모니터 제거, WebView 재시작은 이번 릴리스의 필수 조건으로 확장하지 않는다.

후보 빌드 및 CI는 PR #5와 실행 이력으로 특정하고, Windows 수동 결과가 통과하기 전에는 릴리스를 게시하지 않는다. 로컬 재검증은 프런트엔드26/26, 앱/테스트 TypeScript, production build, rustfmt, diff whitespace 검사 PASS다. 이 결과는 Rust tests/Clippy 또는 Windows GUI PASS를 의미하지 않는다.

## 1. 결론과 제품 목적

Port Lens의 목적은 로컬 개발 포트와 등록 App의 상태를 가볍게 계속 확인하고, 소유권이 검증된 프로세스만 안전하게 제어하는 것이다. Compact는 장식용 축소창이 아니라 상주 모니터다. 따라서 드래그를 매끄럽게 만들기 위해 compact 상태 전체의 polling을 정지하는 것은 해결책이 아니다.

이번 검수는 기존에 제외했던 드래그 잔여 증상, 별도 hover window, Open 복원, 드래그 종료 후 갱신을 다시 조사했다. 현재 구조를 전면 교체할 근거보다는 **제스처 종료·취소·비동기 응답·창 전환 사이의 소유권을 보완할 근거**가 먼저 확인됐다.

직접 수정한 것은 재현 가능한 프런트엔드 경합과 위치 저장의 hover 숨김 부작용이다. 약 3/10 드래그 시작의 짧은 hitch/jump가 사라졌다는 결과는 확보하지 않았다. 네이티브 Open의 부분 실패 복구와 디스플레이 변경 대응은 아래 명세로 분리했다. 기존 v0.3.1의 Windows 수동 PASS를 새 후보의 PASS로 재사용하지 않는다.

## 2. 기준점과 검수 범위

| 구분 | 기준 |
| --- | --- |
| 배포된 기본 브랜치 | `320fe90045ef60d021f88ab9646c8e8a952d5559` |
| v0.3.1 릴리스 커밋 | `e32e9a95018e4556e733cb75b53c1a07e597dc49` |
| 마지막 릴리스 전 Windows 실검증 코드 | `680d18e9044c174e73c6f806b80f45d1a566eadb` |
| 이번 변경의 부모 후보 | `363e77b7baf59a19659b04abf683d34003574450` |
| 작업 브랜치 | `fix/product-audit-2026-09-18` / 기존 Draft PR #5에 후속 변경 |

부모 후보의 managed multi-port Kill 보호, UI 스레드 밖 runtime 조회, 교체된 PID의 오래된 snapshot 제거 방지 등은 유지했다. 이번 검수는 그 변경을 다시 구현하지 않는다. 일반 목적 검수의 후속 A03~A09는 `2026-09-18-product-purpose-audit.md`를 참조한다.

주요 검수 소스는 `src/App.tsx`, `src/CompactHover.tsx`, `src/api.ts`, `src/main.tsx`, `src-tauri/src/bubble.rs`, `src-tauri/src/window_state.rs`, `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`이다. 파일의 행 번호보다 아래 함수명을 추적 기준으로 사용한다.

## 3. 기존 동작에서 반드시 보존할 것

- main은 compact에서 고정 크기의 이동 대상이며, 목록은 별도 `compact-hover` WebviewWindow다. hover를 main의 크기 확장으로 되돌리지 않는다.
- 실제 이동 시작은 기존 4px 임계값 이후다. 실패한 D0.4 pointerdown 전체 gating을 되살리지 않는다.
- Rust가 실행 시점의 현재 커서를 읽는다. 프런트엔드의 과거 screenX/screenY 좌표를 큐에 넣어 재생하지 않는다.
- 이동 중 기존 위치 전용 `SetWindowPos` 경로를 유지한다. 매 move마다 resize, z-order 변경, 설정 파일 저장, React state 갱신을 추가하지 않는다.
- idle targeted polling 약 3초, 전체 inventory 약 10초의 기존 루프를 유지한다. 제스처 종료 후 별도 catch-up을 수행한다.
- expanded 크기의 기존 inner-size 복원, Open의 native 호출 전 mutex 해제, taskbar 정책, 프로세스 소유권 검증을 보존한다.

## 4. 발견 사항과 직접 수정

아래의 ‘재현’은 fake timer와 지연 Promise를 사용한 React/Tauri API mock 테스트다. Windows HWND나 WebView2를 재현했다는 뜻이 아니다.

### CL-01 / P1 — 이전 드래그 완료가 다음 드래그의 polling 정지를 해제

위치: `App::finishBubbleDrag`, `resumeCompactPollingAfterDrag`.

기존 순서: A pointerup의 마지막 move가 대기 → B 드래그 시작 → A의 finally가 무조건 `compactDragActive=false` → B 이동 중 polling/state 반영 재개. 하나의 boolean으로 서로 다른 제스처를 구분할 수 없었다.

수정: 증가하는 drag session을 부여하고, 종료 처리와 catch-up 실행 직전에 현재 session인지 검사한다. 과거 세션의 완료는 새 세션의 gate를 풀지 못한다. 버튼 클릭·취소·Open도 기존 세션을 무효화한다.

회귀: `an old drag completion cannot unpause a newer drag`.

### CL-02 / P1 — 취소와 정상 pointerup이 같은 마지막 이동을 실행

위치: `App::finishBubbleDrag`.

기존에는 pointercancel/lostpointercapture도 커서를 다시 읽는 move를 요청했다. 취소 시점의 커서는 원래 제스처의 최종 위치를 의미하지 않는다.

수정: 정상 pointerup만 마지막 샘플을 요청한다. pointercancel, lostpointercapture, blur, Open 취소에서는 미전송 대기 샘플을 폐기하고 이미 실행 중인 작업이 끝난 뒤 gate를 정리한다. capture는 멱등적으로 해제한다.

회귀: pointercancel/lostpointercapture 각각의 no-final-move, window blur, native/tray Open, move 실패 후 갱신 재개.

한계: 이미 Rust에 전달한 invoke 자체는 취소하지 못한다. 새 제스처와 겹친 과거 네이티브 호출까지 무효화하려면 CL-12의 backend session 검증이 필요하다.

### CL-03 / P1 — Open 뒤 Refreshing…이 영구 잔류할 수 있음

위치: `refreshInventory`의 finally와 `resumeCompactPollingAfterDrag`.

기존 순서: 초기/수동 inventory 요청이 loading=true 설정 → 드래그 중 요청 완료 → finally가 loading 해제를 생략 → 이후 silent refresh도 loading을 해제하지 않음.

수정: foreground 요청의 실제 pending 여부를 ref로 관리하고, 드래그 종료 시 UI loading을 그 상태와 다시 맞춘다. 실제 요청이 아직 끝나지 않았으면 완료된 것으로 위장하지 않는다.

회귀: `settles foreground loading after a refresh completes during drag`.

### CL-04 / P1 — 드래그 전 응답이 종료 직후 다시 반영됨

위치: inventory / monitored / managed refresh의 성공·실패 처리.

기존의 ‘현재 drag 중인지’ 검사만으로는 드래그 전에 시작했지만 해제 후 도착한 응답을 가려내지 못한다.

수정: polling epoch를 요청 시작 시 캡처한다. 실제 drag 시작·강제 취소·unmount가 epoch를 무효화한다. 오래된 값과 오류를 모두 배제하고, managed mutation epoch도 함께 검증한다. forced managed 요청도 최신 in-flight 추적 대상에 포함한다.

회귀: `pre-drag inventory data resolving after release is discarded until catch-up`.

### CL-05 / P1 — 느린 전체 목록이 등록 App의 종료 후 catch-up까지 막음

위치: `resumeCompactPollingAfterDrag`.

기존에는 세 종류의 in-flight Promise가 전부 끝나야 전체 catch-up을 시작했다. 느린 inventory가 작은 monitored/runtime 갱신도 지연시킨다.

수정: inventory lane과 monitored→managed lane을 독립적으로 drain/catch-up한다. 기존 요청과 무작정 중복 실행하지 않되, 전체 inventory 완료를 빠른 lane의 선행조건으로 삼지 않는다.

회귀: `a slow full inventory does not block the post-drag monitored catch-up`.

한계: 이 변경은 OS 조회 자체의 timeout을 구현하지 않는다. targeted 요청 자체가 영구 대기하면 해당 lane도 대기할 수 있다. 일반 감사의 process-query timeout 항목과 연결한다.

### CL-06 / P2 — pointermove마다 무제한 invoke가 누적될 수 있음

위치: 새 `src/compactMovePump.ts`, 기존 App의 pointer handler 연결.

수정: 한 제스처 안에서 in-flight move 하나와 최신 pending 요청 하나만 유지한다. pending에는 좌표가 없고, hover hide 필요 여부만 합친다. 정상 종료는 final sample까지 drain하고, 취소는 pending을 폐기한다. drain과 finally 사이에 들어온 요청도 놓치지 않으며 재시작된 worker까지 종료를 기다린다.

move 오류마다 React 오류 렌더를 일으키지 않고, 제스처 종료 시 보고한다. 실패한 move를 무한 재시도하지 않는다. 새로운 네이티브 이동 방식이나 의도적인 주사율 제한을 추가하지 않는다.

회귀: 100회 pointermove burst, hide flag 보존, cancel이 queued pointerup을 덮어씀, 실패 처리, microtask 경계 drain.

한계: ‘한 개’는 제스처별 제한이다. 이미 전달된 이전 제스처 invoke는 새 제스처와 잠깐 겹칠 수 있다. hitch 감소율은 아직 측정하지 않았다.

### CL-07 / P1 — hover 표시 상태와 실제 창 숨김의 소유권 충돌

위치: `window_state::schedule_persist`.

기존에는 main 이동/크기 변경 뒤 약 400ms 지연 저장 worker가 compact-hover를 숨겼다. 사용자가 그 사이 hover를 열면 프런트엔드 visible 플래그와 무관하게 창이 사라지고, visible=true가 남아 재진입 표시를 막을 수 있다.

수정: 위치 저장 worker에서 hover hide를 제거했다. 저장은 위치 저장만 담당하고, hover 숨김은 포인터 leave/실제 drag/Open 등 수명주기 경로가 담당한다. clamp와 저장 및 taskbar 유지 기능은 제거하지 않았다.

검증: 저장 모듈에 hover 숨김 호출이 다시 들어오지 않는 정적 contract 테스트. 실제 네이티브 타이밍은 Windows 수동 항목 W02로 확인한다.

### CL-08 / P1 — hover show가 진행 중인데 첫 move가 숨김을 생략

위치: `beginBubbleHover`, `moveBubbleDrag`.

show invoke가 아직 응답하지 않으면 visible=false일 수 있다. 기존 첫 move는 visible/panel-inside만 보고 hideHover=false를 보냈다.

수정: in-flight hover show 수도 추적하여 첫 move의 hide 조건에 포함한다. **hover가 닫혀 있고 pending show도 없는 정상 드래그에는 추가 hide 호출을 넣지 않는다.** 이전 generation의 show 완료가 새 generation의 표시를 뒤늦게 숨기지 않도록 정리했다.

회귀: deferred show 중 첫 move는 hide=true, hover-closed move는 hide=false. 네이티브 수준의 역순 실행·문서 재시작까지 해결한 것은 아니다(CL-11).

### CL-09 / P2 — passive revision=0 업데이트가 표시 준비 ACK를 지움

위치: `CompactHover`의 DATA_EVENT listener / layout effect.

동일 React batch에서 표시 요청 revision=N 이후 live revision=0이 들어오면 최종 state가 0이 되어 RENDERED ACK가 생략될 수 있다.

수정: 데이터는 최신 payload를 사용하되 현재 문서 내 acknowledgement revision은 감소시키지 않는다. main의 READY timeout 반환값도 실제로 확인한다. 숨겨진 hover에는 주기적인 passive payload 전송을 하지 않고 최신 ref만 보관한다.

회귀: revision7과 revision0을 한 batch에 전달해 ACK7을 확인. 전체 WebView 재시작 프로토콜은 이번 변경 범위가 아니다.

### CL-10 / P1 — Open과 비동기 mode 초기화의 수명주기 부족

위치: `expandBubble`, bubble-state listener effect, compact interaction cleanup.

수정: Open은 프런트엔드에서 single-flight로 동작하며 진행 중인 capture/pending movement를 먼저 정리한다. native/tray Open 이벤트로 compact DOM이 제거되는 경우에도 monitoring gate를 정리한다. listener를 먼저 등록한 뒤 초기 state를 읽고, 그 사이 더 최신 mode 이벤트가 왔으면 오래된 조회 응답을 버린다. unmount 후 설치된 listener는 즉시 해제한다. StrictMode effect replay로 무효화한 초기 snapshot은 재조회하며, blur와 Open이 연달아 취소할 때는 같은 in-flight move가 끝날 때까지 기다린다.

회귀: Open 연타, 실패 후 프런트엔드 재시도 가능, native Open, stale initial snapshot, 늦은 listener cleanup.

**중요:** mock의 Open 재시도 PASS는 아래 N01 네이티브 복원 실패의 해결을 의미하지 않는다.

## 5. 직접 고치지 않은 네이티브/프로토콜 작업 명세

### N01 / P1 — 네이티브 Open 부분 실패 복구 (최우선 후속)

소유자: `bubble.rs::expand`, `collapse`, `BubbleState`; 호출자는 `lib.rs`의 Open/트레이/설정 변경 진입점.

발견: expand는 mutex 안에서 `collapsed=false`를 먼저 기록한 뒤, mutex를 해제하고 여러 native 설정을 적용한다. 이 중 하나가 실패하면 UI는 compact 상태로 남을 수 있지만 backend는 이미 expanded다. 다음 expand는 전체 복원 대신 show/unminimize/focus 경로만 실행하므로 복원 geometry를 다시 적용하지 않을 수 있다.

요구 계약:
1. `Expanded / Collapsed / Transitioning` 또는 동등한 명시적 전이 상태를 기존 BubbleController 안에 둔다. 별도 경쟁 window manager를 만들지 않는다.
2. 전이 시작 시 snapshot·transition id·목표 mode만 짧게 잠가 확보한다. native window API를 mutex를 잡은 채 호출하지 않는다.
3. 정상/최대화 복원 정보와 원래 topmost 값을 성공 전까지 보존한다. 중간 실패 시 동일 목표를 재시도할 수 있어야 한다.
4. 전이 중 move, hover show, persistence, taskbar keeper의 정책을 정의한다. 중간 geometry를 정상 expanded bounds로 저장하지 않는다.
5. geometry/style 복원 성공 후에만 최종 mode를 commit/emit한다. focus 실패는 geometry 실패와 구분하여 사용자에게 알려야 하며, 실패 때문에 완료된 geometry를 compact로 오인하지 않는다.
6. 반복 Open은 현재 전이에 합류하거나 일관되게 no-op 처리한다. 새 목표와 충돌하면 명시적으로 직렬화한다. native 경로의 중복도 막아야 한다.
7. 기존 inner-size 복원과 expand의 lock-drop-before-native 원칙을 유지한다. collapse도 긴 mutex 보유를 제거하되, 성공 전 collapsed publication 순서를 검증한다.

구현 순서: 순수 상태 전이 테스트 → native 연산 fault-injection용 얇은 기존 함수 경계 → 각 API 실패 후 retry 테스트 → Windows 실검증. 모든 API를 전면 추상화하는 프레임워크는 만들지 않는다.

수용 기준: decorations/size/position/show/focus 각각에 실패를 주입하고 다음 Open으로 복구 가능; 잠금 중 native 호출 없음; 실패로 permanently split-brain 상태가 남지 않음; 일반 Open/트레이 Open/설정에 의한 복원 동일 정책; 30회 compact↔Open 후 inner size 불변.

### N02 / P1·P2 — 화면 제거, DPI 변경, 최대화 이전 위치 복원

소유자: `bubble.rs`의 ExpandedWindow capture/expand와 `window_state.rs`의 기존 fit/visibility helper.

발견: startup은 work-area fit을 하지만 compact Open은 저장한 physical position/inner size를 직접 적용한다. compact 동안 모니터가 제거되면 이전 위치가 유효하지 않을 수 있다. 최대화 상태에서 저장한 inner_size가 normal restore bounds와 같다고 볼 수도 없다. 정적 경로 위험이며 실제 Windows 재현 결과는 아직 없다.

명세: 저장 위치의 모니터 유효성을 재검사하고 현재/primary 대체 모니터를 선택한다. existing fit/clamp를 재사용하되 전이 완료 전 persistence를 막는다. normal restore bounds와 maximized flag를 분리 보관한다. 무조건 논리 크기/물리 크기 중 하나로 통일하기보다 각 저장 필드의 단위와 DPI 변환 시점을 명시한다.

수용 기준: 음수 좌표 모니터, 100/125/150/200% 배율, compact 중 모니터 분리, 800×580보다 작은 작업영역, maximize→compact→Open→unmaximize, 반복30회에서 창 접근성·normal bounds·inner-size drift를 확인한다. taskbar 예약영역을 허용하는 compact 정책과 expanded work-area 정책을 혼동하지 않는다.

### N03 / P2 — 오래된 taskbar keeper의 Open 이후 topmost 재적용

소유자: `start_taskbar_z_order_keeper`, `refresh_taskbar_z_order`.

발견: keeper가 generation/collapsed를 검사한 뒤 native 조회와 SetWindowPos 사이에 Open이 완료되면, 오래된 작업이 expanded 창에 영향을 줄 수 있는 check/use 간격이 있다. 현재 generation 취소는 다음 loop를 막지만 이미 통과한 작업까지 보장하지 않는다.

명세: 실행이 실제 적용되는 window/UI owner에서 transition id와 generation을 다시 검증한다. 무효 keeper 작업은 no-op. mutex를 잡은 채 native API를 호출하거나 단순 반복 topmost 강화로 덮지 않는다.

수용 기준: keeper를 check 이후 일시 정지→Open 완료→keeper 재개 시 원래 expanded topmost 값 보존. 포커스 탈취 없음. compact가 taskbar 아래로 사라지지 않는 기존 gate도 재확인한다.

### CL-11 / P2 — hover document/session 재시작과 순서 복구

소유자: main의 hover coordinator와 `CompactHover`의 READY/DATA/RENDERED 프로토콜.

이번 revision 단조증가는 동일 문서 생명주기의 batch 경합만 해결한다. main만 reload되어 revision이 초기화되고 hover가 살아 있는 경우, 또는 hover만 재시작된 경우까지 검증하지 않았다.

명세: 문서/session epoch와 revision을 함께 전달한다. listener 준비 후 HELLO/READY를 교환하고, 재시작한 한쪽이 현재 payload를 다시 요청할 수 있게 한다. show/hide에도 intent generation을 연결하여 오래된 ACK/show/hide가 최신 상태를 바꾸지 못하게 한다. timeout은 bounded retry와 오류 상태로 표현하고 unmount cancellation은 성공 ACK로 취급하지 않는다.

수용 기준: main-only reload, hover-only reload, READY 유실, ACK 유실, passive update와 show 병합, 늦은 show 완료, rapid leave→reenter→drag→Open, pending waiter 정리. 부정확한 ACK를 보내기 위한 임의 sleep 증가는 금지한다.

### CL-12 / 조건부 — 이미 전달된 이동의 세션 무효화와 잔여 hitch 계측

현재 JS pump는 미전송 샘플만 버린다. Windows 검증에서 오래된 invoke의 실제 이동이 다음 gesture 또는 Open에 영향을 주는 증거가 나오면 기존 `move_compact_bubble`에 최소 session 검증을 추가한다. `start/move/end`의 native owner는 하나여야 하고, capture loss·Open·settings disable 시 session을 무효화한다. stale move는 cursor 조회와 SetWindowPos 이전에 no-op 해야 한다.

단순히 드래그 잔여 증상이 있다는 이유로 D1 native SetCapture 시스템 전체를 바로 추가하지 않는다. D1은 frontend scheduling/IPC가 병목으로 입증되고 기존 architecture로 목표를 만족하지 못할 때 별도 PoC로 판정한다.

## 6. 드래그 잔여 증상 검증 계획

비교군은 동일 Windows PC·같은 App/port 수·같은 polling 조건으로 고정한다.

| 비교군 | 구분하려는 효과 |
| --- | --- |
| v0.3.1 | 기존 약3/10 잔여 증상의 재현 기준 |
| `363e77b` | 이전 PR의 runtime 조회 UI 스레드 분리 효과 |
| 이번 compact lifecycle 후보 | session/epoch, latest-only move, hover ownership 효과 |

각 비교군에서 최소100회 드래그를 수행한다. hover 닫힘/열림, 3초·10초 갱신 경계, idle/실제 상태 변화, 첫 실행/반복 실행을 나눠 기록한다. 시작 hitch 횟수만 세지 말고 지속 이동의 pause/catch-up, cursor offset, 종료 후 stale 표시, Open 회복을 각각 분리한다.

계측 명세(이번에 구현하지 않음): gesture id, threshold 통과 시각, 첫 invoke 대기시간, native 이동 처리시간, 최종 move drain시간, monitored catch-up 완료시간. 프런트엔드 performance.now와 native monotonic clock의 절대값을 직접 빼지 않는다. 각 clock 안의 duration과 공통 id를 기록한다. 매 pointermove마다 파일에 쓰지 않고 bounded memory에 모아 제스처 종료 시 요약한다. p50/p95/max와 오류·취소 사유를 보고한다.

‘hitch 해결’ 판정은 Windows 반복 실측 이후에만 한다. 제안 최소 gate는 지속 offset/중간 catch-up jump 0회, 취소 후 불필요 이동 0회, polling 복구 누락 0회다. 시작 지연의 수용값은 baseline 실측과 사용자의 체감 결과를 함께 두고 결정한다. 임의 ms 목표를 기존 합의로 꾸미지 않는다.

## 7. Windows 실행 체크리스트

| ID | 동작 | 통과 조건 |
| --- | --- | --- |
| W01 | compact idle에서 외부 listener 시작/종료 | Open 없이 상태가 기존 polling cadence로 반영 |
| W02 | compact 전환 직후 hover, drag 후 빠르게 leave/reenter | 지연된 위치 저장 때문에 hover가 사라지거나 visible flag가 고착되지 않음 |
| W03 | hover 목록 0/1/8/9개 이상, 긴 이름 | 빈 창 없음, 8행 cap·scroll·내용 clipping 정상 |
| W04 | 20초 이상 연속 drag, 그 사이 상태 변경 | 이동 중 render 재개 없음, release 후 monitored lane catch-up |
| W05 | 느린 전체 inventory 상태에서 release | 전체 inventory를 기다리느라 빠른 App 갱신이 함께 막히지 않음 |
| W06 | A release 직후 B drag 반복 | A finalizer가 B polling gate를 해제하지 않음 |
| W07 | capture loss/Alt-Tab/취소 | 추가 final cursor-follow 이동 없음, capture/polling 정리 |
| W08 | hover show 도중 drag 시작 | 목록이 이전 위치에 남지 않음; hover-closed drag는 불필요 hide 없음 |
| W09 | Open 연타, tray Open, compact 설정 해제 | 중복 복원/멈춤/transparent ghost 없음, polling 유지 |
| W10 | compact↔Open 30회 및 maximize round trip | inner size 증가 없음, 원래 위치·normal bounds 보존 |
| W11 | taskbar 겹침, 모니터 변경/분리, 혼합 DPI | topmost·cursor ratio·저장 위치·창 접근성 보존 |
| W12 | 정상 종료 후 재실행 | 마지막 compact 위치 복원, managed ownership 기능 무회귀 |

W10/W11의 새 실패는 N01~N03 기준으로 별도 수정한다. unit/static PASS나 Windows compilation PASS를 이 표의 수동 PASS로 치환하지 않는다.

## 8. 구현 파일과 자동 검증

변경 파일: `src/App.tsx`, `src/CompactHover.tsx`, `src/compactMovePump.ts`, `src-tauri/src/window_state.rs`. 검증 연결: `tests/compact-window.test.tsx`, `tests/compact-move-pump.test.ts`, `tests/compact-source-contracts.test.ts`, `vitest.config.ts`, `tsconfig.test.json`, package manifest/lock, CI workflow.

기존 `bubble.rs`, 이동 API payload, native 이동 primitive, 프로세스 제어 코드는 이번 후속 변경에서 수정하지 않았다. 테스트용 Vitest/jsdom은 개발 의존성이다. 기존 lockfile 패키지의 버전 변경은 0건, 추가된 테스트 관련 패키지는 58개다.

| 검증 | 이번 후보 판정 |
| --- | --- |
| React mock / move pump / static 계약 테스트 | **26/26 PASS**, 최종 로컬 실행 2026-09-18 01:45 KST |
| 앱+테스트 TypeScript 검사 | **PASS**, `npm test` 선행 단계 |
| frontend production build | 로컬 PASS |
| rustfmt check | 로컬 PASS |
| 로컬 cargo test | **환경 차단**: 컴파일 중 ENOSPC, exit101. 테스트 실행 완료 아님 |
| 로컬 Clippy | 앞 단계 차단으로 이번 실행에서는 미실행 |
| Windows/macOS GitHub CI | 이번 변경은 원격 미반영이므로 새 CI 실행 없음 |
| Windows 실제 GUI | **미실행** |

로컬 검증 중 이 worktree에서 생성된 `src-tauri/target` 약1.3GiB를 `cargo clean --manifest-path src-tauri/Cargo.toml`로 정리했다. 다른 프로젝트나 기존 사용자 데이터는 삭제하지 않았다. 저장공간 부족으로 실패한 테스트 파일 수정은 원본이 유지된 것을 확인한 뒤 다시 적용하고 타입/회귀 검사를 실행했다.

재실행 명령:
```sh
npm ci
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Rust 재검증 전 충분한 디스크 여유를 확보한다. 과거41/41 PASS는 부모 후보363e77b의 결과이며 이번 변경의 성공 증거로 승격하지 않는다.

## 9. 인수인계 및 병합 순서

현재 후보는 기존 Draft PR #5 브랜치의 로컬 미커밋 변경이다. 커밋·push 요청이 도구에서 차단되어 새 SHA와 원격 반영은 없다. 다음 작업자는 이 작업트리의 modified/untracked 파일을 먼저 확인하고 검토 후 commit/push하여 PR에 반영한다. main/v0.3.1은 교체하지 않는다. 우선 이 문서의 프런트엔드 회귀와 해당 SHA의 CI를 확인하고 W01~W12를 실행한다. Native Open failure recovery(N01)는 별도 작은 변경으로 구현·검증하고, display restoration(N02), keeper ordering(N03), hover session recovery(CL-11)는 각각 독립 증거를 남긴다.

잔여 hitch는 runtime-only 비교군과 이번 후보를 구분하여 측정한다. 최신 후보가 악화되면 이번 후속 커밋만 되돌리고 이전 managed-runtime 안전성 수정까지 제거하지 않는다. 이미 존재하는 문서의 accepted residual은 배포된 baseline의 과거 판단으로 남기되, 이번 후보의 새 검증 상태와 분리한다.

## 10. 근거

저장소 이력:
- [기존 D0~D0.5 비교·실검증 기록](./2026-09-15-compact-drag-poc-d-audit.md)
- [최초 compact drag/hover 감사](./2026-09-15-compact-drag-hover-audit.md)
- [이전 일반 목적 감사](./2026-09-18-product-purpose-audit.md)
- [Draft PR #5](https://github.com/teeeeooo/port-lens/pull/5)

공식 외부 근거(2026-09-18 확인):
- [Tauri: Calling Rust / Async Commands](https://v2.tauri.app/develop/calling-rust/#async-commands): sync command와 async 작업 실행 경계를 구분해야 한다. 기존 runtime 조회 worker 분리는 이 원칙과 일치하지만 그 자체가 hitch 제거의 실측 증거는 아니다.
- [W3C Pointer Events](https://www.w3.org/TR/pointerevents3/): pointerup, pointercancel, lostpointercapture는 구별되는 이벤트다. 취소에도 최종 이동을 해야 한다는 요구는 없다.
- [Microsoft SetWindowPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos): 위치·크기·z-order·활성화 flag를 독립적으로 다룬다. 기존 position-only 동작을 보존하고 worker/native ordering은 별도로 검증한다.
