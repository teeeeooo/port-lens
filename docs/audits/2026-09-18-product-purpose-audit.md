# Port Lens 목적 적합성 검수 및 안정화 작업 명세

작성일: 2026-09-18 (Asia/Seoul)
검수 기준: `main` / `320fe90045ef60d021f88ab9646c8e8a952d5559`
수정 브랜치: `fix/product-audit-2026-09-18`
상태: 제한된 수정 구현 및 macOS 로컬 검증 완료. Windows GUI 실검증과 병합은 미완료.

## 1. 결론과 제품 목적

전체 재설계보다 기존 구조를 유지한 안정화가 적합하다. Port Lens의 목적은 “어떤 프로세스가 포트를 사용하고 있는가, 그것이 내가 등록한 앱인가, 안전하게 제어할 수 있는가”를 적은 조작으로 확인하는 것이다. 새 기능 수보다 상태의 신뢰성, 제어 권한의 정확성, 가벼운 상주 동작이 우선이다.

전체 TCP inventory의 느린 갱신과 등록 App의 집중 갱신, 외부 listener의 monitoring-only 등록, 검증된 런타임에만 Stop/Restart를 허용하는 경계는 유지한다. `v0.3.1`의 separate compact-hover window, Open 복원, 드래그 종료 후 catch-up, 창 위치/DPI 처리는 이번 수정 대상이 아니다. 수용된 D0.5 드래그 잔여 증상은 다시 열지 않는다. UDP 및 HTTP health check도 이번 안정화의 선행 조건이 아니다.

이번 결과는 핵심 경로의 소스 검수와 macOS 테스트 결과다. Windows에서 실제 사용자 프로세스가 잘못 종료되었다거나 설정 파일이 유실되었다는 실측 보고가 아니다. 아래의 오류·경합 시나리오는 코드에서 확인한 경로와 추가 검증이 필요한 재현 조건을 구분하여 기록한다.

## 2. 검수 범위 및 근거 읽는 법

README 양 언어, STATE/BACKLOG, CI와 패키징 설정, React polling/action/status 경로, Rust listener scan, runtime identity/reattach/lifecycle, registry/settings/logging을 대조했다. 창 관리 구조 및 진입 경로는 확인했지만 Windows native drag 구현 전체의 재감사나 GUI 실검증은 수행하지 않았다. 의존성 전체의 공급망 보안 감사도 아니다.

이 문서의 원본 코드 줄 번호는 모두 위 기준 SHA를 가리킨다. 수정 후 lib.rs의 줄 번호는 달라진다. 재현하지 않은 현상은 실제 장애가 발생했다고 해석하지 않는다.

| ID | 우선순위 | 항목 | 이번 처리 |
|---|---|---|---|
| A01 | P1 | 관리 child의 다른 포트를 선택한 Kill이 보호 검사를 통과 | 백엔드 수정 + 정책 회귀 테스트 |
| A02 | P1 | 동기 runtime 조회가 UI thread에서 tasklist/ps 실행 | async worker + snapshot 적용 보호 + 테스트 |
| A03 | P1 | listener scan 오류/잘못된 출력이 정상 empty로 해석될 가능성 | 후속 구현 명세 |
| A04 | P1 | registry/settings 비원자적 저장, 실패 후 메모리 불일치 | 후속 구현 명세 |
| A05 | P1 | 동시 Start 및 Start 중 Edit/Remove의 경합 | 후속 구현 명세 |
| A06 | P1 | stale 데이터를 정상 Online으로 표시하고 오류를 다른 조회가 지움 | 후속 구현 명세 |
| A07 | P2 | 앱 identity, 집계, 상세 목록 표현의 불일치 | 후속 구현 명세 |
| A08 | P1/P2 | process probe의 오류/종료 구분 및 시간 제한 부족 | A02와 분리한 후속 명세 |
| A09 | P2 | 장시간 실행 App의 stdout/stderr가 무제한 증가 가능 | 자동 수집 제거로 방향 변경(PR #6) |
| A10 | P2 | README의 닫기 동작 설명이 실제 구현과 다름 | EN/KO 문서 수정, 런타임 동작 불변 |

P1은 다음 안정화에서 우선 처리할 정확성·안전성 문제이며, 인터넷 원격 취약점 등급이나 현재 사용자 피해를 뜻하지 않는다. P2는 그 다음의 일관성·상주 운영 개선이다.

## 3. 직접 수정한 내용

### A01. 관리 프로세스의 보조 포트 종료 방지

원본 `src-tauri/src/lib.rs:1048-1107`은 선택한 포트 하나만 다시 스캔한 후, 그 결과로 대상 PID가 관리 App 포트도 소유하는지 검사한다. 관리 런타임의 root는 cmd.exe이고 listener는 child일 수 있으므로 root PID 비교만으로는 충분하지 않다.

예: App A의 등록 포트는 3000, root PID는 4100, child PID는 4200이다. 4200이 3000과 3001을 함께 열면 3001에서 Kill을 선택할 때 기존 targeted scan에는 3000이 없다. 따라서 관리 포트 보호 검사에서 빠질 수 있다. UI도 등록 포트 중심으로 Kill을 비활성화하므로 백엔드에서 막아야 한다.

변경: 명시적인 Kill 직전에만 fresh full inventory를 읽는다. `validate_unmanaged_termination`이 선택 PID+port의 유효성, 관리 root 여부, 같은 PID의 다른 listener가 현재 관리 포트인지 검사한다. 일반 3초 monitored polling은 targeted 방식 그대로다. 종료 명령은 잠금을 해제한 후 blocking worker에서 실행한다.

추가 테스트: `unmanaged_kill_rejects_secondary_port_of_managed_child`, `unmanaged_kill_rejects_managed_root_and_stale_selection`, `unmanaged_kill_allows_monitoring_only_listener`, `unmanaged_kill_does_not_confuse_unrelated_process_with_managed_owner`.

범위 한계: 모든 ancestor/descendant 간접 종료나 PID 재사용, 검사와 종료 사이의 시간차까지 원천 차단한 변경은 아니다. 별도 child가 보조 포트만 열거나 선택 PID의 자식 중 관리 런타임이 있는 경우 등은 추가 tree-identity 보호가 필요하다. 테스트는 정책 함수와 fixture를 검증하며 Windows의 taskkill 실동작을 검증하지 않는다.

### A02. runtime polling의 UI thread 차단 제거

원본 `lib.rs:435-460`의 `get_managed_runtimes`는 동기 Tauri command 안에서 runtime mutex를 잡고 PID별 `tasklist` 또는 `ps`를 실행한다. 실제 Windows의 지연 시간은 이번에 측정하지 않았지만, 동기 command의 무거운 작업이 UI thread를 차지하는 구조는 확인했다.

변경: command를 async로 바꾸고 짧은 잠금에서 runtime snapshot을 복사한다. process probe는 `spawn_blocking`에서 잠금 없이 실행한다. 복귀 후에는 조회 당시 `(appId, PID)`와 현재 값이 같은 항목만 제거한다. 그 사이 Restart로 바뀐 PID나 새로 추가된 App을 늦은 조회가 지우지 않는다. 반환 목록은 appId로 정렬한다.

추가 테스트: `delayed_runtime_probe_cannot_remove_replacement_or_new_runtime`는 실제로 끝난 항목만 제거하고 교체·신규 runtime을 보존하는지 검사한다.

범위 한계: tasklist 실행 자체를 native API로 교체하거나 모든 probe에 timeout/Unknown 상태를 추가한 것은 아니다. 다른 start/stop/reattach 경로의 동기 조회, 실패를 false로 처리하는 정책은 A08에 남아 있다. 기존 드래그 잔여 증상이 해결됐다고 주장하지 않는다.

### A10. 닫기/최소화 문서 정정

원본 `lib.rs:1433-1445`의 CloseRequested는 Port Lens를 종료한다. README는 tray로 숨긴다고 설명하고 있었다. EN/KO 문서를 실제 동작인 `Minimize → Compact 또는 Tray`, `X 또는 Quit → Port Lens 종료`로 수정했다. 안정화된 창 동작 자체는 변경하지 않았다.

## 4. 검증 기록과 현재 변경 경계

| 검증 | 결과 |
|---|---|
| 원본 main snapshot의 npm ci / frontend build | PASS |
| 원본 macOS Rust 테스트 | 36 passed, 0 failed |
| 변경 후 cargo fmt | 완료 |
| 변경 후 Clippy, all-targets/all-features, -D warnings | PASS |
| 변경 후 macOS Rust 테스트 | 41 passed, 0 failed |
| 변경 후 frontend production build | PASS, 기존 frontend 소스 불변 |
| git diff --check | PASS |
| Windows CI | PR의 해당 commit checks를 별도로 확인할 것 |
| Windows GUI, taskkill 실사용, 성능 비교 | 미실행 |
| 새 NSIS/MSI/Portable 릴리스 | 생성·배포하지 않음 |

검증 명령은 `npm run build`, `cargo fmt --manifest-path src-tauri/Cargo.toml`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`, `cargo test --manifest-path src-tauri/Cargo.toml`이다. 로컬 검증에는 기존 checkout의 Cargo target cache를 재사용했으며 수정은 별도 worktree에서만 수행했다.

실행 코드 변경은 lib.rs에 한정한다. React, IPC 인자/응답 형태, Cargo/npm 의존성, OS scan script, bubble.rs, window_state.rs, release version/tag는 변경하지 않는다. main 및 기존 릴리스를 교체하는 작업은 포함하지 않는다.

## 5. A03 — scan 결과의 신뢰성 및 UTF-8 계약

**근거:** `ports.rs:74-135,137-205`의 Get-NetTCPConnection은 SilentlyContinue를 사용하지만 query 오류를 별도로 분류하지 않는다. pipe reader는 read_to_string의 오류를 무시하고, parser는 빈 문자열/null을 빈 listener 목록으로 받아들인다. `process_control.rs:79-198`의 ancestry 출력도 유사한 pipe reader를 사용한다.

**문제 조건:** OS query가 실패했는데 뒤의 JSON 출력이 정상 종료하거나, 비 UTF-8/잘린 출력의 읽기 실패가 숨겨지면 “조회 실패”가 “포트 없음”과 구분되지 않을 수 있다. Start의 포트 충돌 확인에까지 이 결과를 쓰므로 표시 문제에만 한정되지 않는다. 한국어 경로가 실제 깨지는지는 해당 Windows PowerShell 환경에서 별도 확인해야 한다.

**구현 계약:** PowerShell stdout JSON의 UTF-8을 명시하고, query 오류를 수집한다. 정상 no-match와 권한·provider·module 오류를 구분하고 후자는 nonzero/error로 전파한다. 빈/null stdout을 정상 빈 결과로 관용 처리하지 않고 정상 empty는 명시적 JSON 배열로 표현한다. Rust reader는 I/O/디코딩/reader-thread 오류를 Result로 전파한다. cmdline enrichment만 실패했다면 listener 목록은 유지하되 metadata unavailable로 분리한다. unknown scan으로 Start/파괴적 제어의 사전 검사를 통과시키지 않는다.

**주의:** 단순히 모든 SilentlyContinue를 Stop으로 바꾸지 않는다. 실제 Windows에서 없는 포트의 query가 어떤 오류 형태를 반환하는지 테스트한 후 필요한 no-match만 제한적으로 허용한다. 한글 오류 메시지 문자열에 의존하지 않는다. timeout 시 child와 reader의 정리 정책도 명시한다.

**대상 파일:** ports.rs, process_control.rs, 필요 시 scan 결과 DTO(models.rs/lib.rs/types.ts/api.ts). command line은 민감한 인자를 포함할 수 있으므로 실패 로그에 원문 전체를 무분별하게 추가하지 않는다.

**완료 기준:** Windows powershell.exe에서 실제 empty scan, 여러 포트 중 일부만 존재, 비관리자 권한 제한, 의도적인 provider 실패, 비 UTF-8/잘린 JSON, stdout/stderr 혼입, 한국어·공백 경로, query timeout을 검증한다. 정상 empty와 실패가 서로 다른 결과여야 하며 Start가 실패한 검사 후 spawn하지 않아야 한다. 생성 script의 문자열 검사만으로 완료 처리하지 않는다.

## 6. A04 — registry/settings 저장의 원자성 및 실패 복구

**근거:** `registry.rs:22-67`은 parse/read 실패 시 빈 App 목록을 만들고, 저장은 fs::write로 원본을 직접 덮어쓴다. `settings.rs:95-168,213-219` 및 lib.rs의 save/remove/observe 경로는 메모리 상태를 바꾼 다음 파일을 저장한다. 실패하면 호출은 Err지만 메모리 상태는 이미 바뀔 수 있다. 일부 설정의 동일값 early-return은 재시도 저장까지 생략할 수 있다.

**구현 계약:** 후보 값 생성 → 검증 → 디스크 commit → 메모리 publish 순서로 처리한다. 동일 디렉터리의 임시 파일에 직렬화·flush/sync 후 OS에 맞는 atomic replace를 사용한다. 원본을 먼저 삭제하는 구현은 금지한다. 쓰기 실패 시 이전 메모리/파일을 유지하고 구체적인 오류를 반환한다. 손상된 파일은 조용히 빈 값으로 덮어쓰지 말고 원본 보존/quarantine 및 복구 상태를 노출한다. 최신 정상 backup, schema validation 및 제한된 복구 절차를 정의한다.

**호출 경로:** App 등록·수정·삭제뿐 아니라 listener 관찰, managed identity 저장, legacy monitored-port migration, window bounds/compact position 저장 모두 같은 정책을 따른다. 일반 목록 조회가 부가 메타데이터 저장 실패 때문에 전체 모니터링까지 잃지 않도록 read와 enrichment persistence의 실패 범위를 나눈다.

**대상 파일:** registry.rs, settings.rs, lib.rs. 공통 persistence helper는 위 두 store의 최소 요구만 추출하고 새 저장 프레임워크로 확장하지 않는다.

**완료 기준:** write/flush/replace 각각의 fault injection, 읽기 전용 경로, 손상 JSON, 재시작/재로딩, 반복 같은 값 저장, migration 중 실패를 테스트한다. 실패 전후 App 목록과 설정 값이 변하지 않아야 하며, 원본 또는 복구 가능한 이전 정상본이 남아야 한다. Windows 열린 파일·권한 특성까지 확인한다.

## 7. A05 — App별 lifecycle 작업 직렬화

**근거:** `lib.rs:781-889`의 Start는 runtime 부재 확인과 listener scan/spawn 사이에 await 경계가 있지만 App별 operation gate가 없다. `App.tsx:786-809`의 busy는 전역 단일 문자열이므로 다른 App 작업이 기존 작업의 busy 표시를 덮을 수 있다. 시작 중에는 runtime이 아직 없어 Edit/Remove도 통과할 수 있다.

**구현 계약:** UI의 버튼 비활성화가 아니라 backend의 App별 gate를 권위로 둔다. 같은 App의 Start/Stop/Restart/Save/Remove를 하나의 operation ID/generation으로 직렬화하거나 명시적인 busy 오류로 거절한다. 다른 App끼리는 불필요하게 전역 직렬화하지 않는다. std::sync::Mutex guard를 await 동안 잡지 않는다. 정상 종료·오류·취소에서 gate가 반드시 해제되게 한다. 설정 snapshot, runtime 등록, watcher 종료 이벤트, reattach suppression에도 같은 generation을 적용한다.

**Frontend:** busy를 App 또는 action key별 Map/Set으로 바꾸고 완료한 작업만 제거한다. Start 중 Edit/Remove와 중복 Start를 막되 backend 검증을 대체하지 않는다. optimistic UI는 성공 응답/최신 generation과 정합성을 유지한다.

**대상 파일:** registry.rs/lib.rs, App.tsx, 필요 시 types.ts와 신규 operation helper. 기존 root creation-time/ancestry 확인 및 reattach suppression 규칙을 삭제하거나 완화하지 않는다.

**완료 기준:** 테스트 barrier로 두 Start가 동시에 scan을 통과하려는 순서를 재현한다. 실제 spawn은 최대 한 번이어야 하고 추적되지 않는 child가 남지 않아야 한다. Start 중 Edit/Remove, Stop+Restart, 서로 다른 App의 병렬 작업, spawn 실패 후 재시도, 늦은 watcher/probe 응답을 검증한다. 프런트 버튼 테스트만으로 완료 처리하지 않는다.

## 8. A06 — freshness와 오류를 상태의 일부로 모델링

**근거:** `App.tsx:184-274`에서 inventory 조회 성공은 공유 error를 무조건 지운다. monitored 조회 실패는 기존 배열을 유지하지만 stale 여부를 표시하지 않는다. `App.tsx:872-916`의 compact return에는 오류 상태가 표시되지 않는다.

**구현 계약:** inventory, monitored listeners, runtime 상태 각각에 lastSuccessAt, pending, error, freshness/generation을 둔다. last-good 데이터는 유지하되 최신 조회에 성공한 것으로 표현하지 않는다. 최초 미조회는 Unknown, 성공한 empty만 Offline 근거로 쓴다. 다른 source의 성공으로 기존 오류를 지우지 않는다. action 오류와 scan 오류도 분리한다. timestamp의 임계값은 현재 3초/10초 루프와 실제 scan duration을 고려해 명시하고 테스트한다.

**표현:** dashboard는 어느 갱신이 실패했는지와 마지막 성공 시점을 보여준다. compact/hover에는 오래된 녹색 Online과 구별되는 작은 상태 표시 및 tooltip을 둔다. 사용자의 상주 모니터 목적을 훼손하지 않도록 compact 전체 polling을 꺼서 문제를 회피하지 않는다. UI freshness는 backend destructive-action 재검증을 대체하지 않는다.

**대상 파일:** App.tsx, CompactHover.tsx, types.ts, i18n.ts, 최소 CSS 및 필요 시 api.ts. 상태 계산을 순수 함수로 분리해 frontend 테스트를 추가하고 CI에서 실행한다.

**완료 기준:** fake timer/지연 Promise로 inventory 성공+monitored 실패, 첫 조회 실패, 복구, 오래된 결과의 역순 도착, action 오류 중 inventory 성공, drag 중 결과 보류 및 drag-end catch-up을 검증한다. dashboard와 compact가 같은 freshness 의미를 사용해야 한다.

## 9. A07 — identity와 표시 일관성

**근거:** `App.tsx:415-535`는 runtime과 같은 포트의 listener가 있으면 Running으로 분류한다. onlineAppCount는 identityChanged도 포함하지만 hover는 제외한다. 첫 listener를 port로만 대표 선택하며, inventory scan은 cmdline enrichment를 생략하여 README의 미등록 runtime 추정 label이 일반 inventory에서는 나오기 어렵다. listener 개수를 ports로 표시하면 IPv4/IPv6별 중복 endpoint를 포트 수로 오해할 수 있다.

**구현 계약:** port occupancy, registered identity match, verified managed ownership, freshness를 분리한다. 프로세스 이름이나 command line의 대략적인 일치만으로 Stop 권한을 부여하지 않는다. 동일 port의 여러 bind/PID를 감추지 않고 충돌/복수 owner로 표현한다. App online 집계와 hover가 공통 selector를 사용하게 한다. listener count와 unique port count의 명칭을 구분한다. managed root가 살아 있지만 listener가 생기지 않는 상태에는 시작 지연/포트 미확인 상태와 Logs 진입을 제공한다.

미등록 App의 friendly label은 기능을 유지한다면 요청 시 또는 관심 listener 중심의 bounded metadata cache로 보완한다. 모든 inventory poll마다 전체 CIM enrichment를 추가하지 않는다. 범위를 줄인다면 README도 실제 동작에 맞게 바꾼다. HTTP health/서비스 준비 완료까지 암묵적으로 의미를 확장하지 않는다.

**완료 기준:** 다른 프로세스가 등록 포트를 점유, 동일 port의 IPv4/IPv6 다른 PID, root만 생존, 정상 reattach, monitoring-only App, metadata 없음, stale 상태에서 dashboard/hover/제어 권한이 일관적이어야 한다.

## 10. A08 — process query/종료의 남은 안전성

**근거:** `process_control.rs:200-253`의 tasklist/taskkill 실행에는 scan과 같은 명시적 timeout이 없고, is_process_alive는 실행 오류를 false로 반환한다. terminate_tree는 종료 명령이 실패해도 이어진 alive 검사에서 false가 나오면 성공으로 처리할 수 있다. A02는 실행 위치와 lock 경계를 개선했을 뿐 이 결과 의미를 바꾸지 않았다.

**구현 계약:** Alive/Exited/Unknown을 구분하고 query 오류를 죽은 프로세스의 증거로 사용하지 않는다. 모든 subprocess 명령에 bounded timeout과 정리 정책을 적용하거나 Windows process handle 기반 API로 바꾼다. alive 검사 실패로 runtime authority를 조용히 삭제하지 않는다. 종료는 실제 종료 또는 이미 종료된 동일 generation을 확인한 경우에만 성공 처리한다. Start/Stop/reattach의 나머지 동기 query도 event thread 및 장시간 lock 밖에서 수행한다.

추가 hardening에서는 unmanaged termination의 descendant 영향 및 PID generation을 확인한다. 단순히 조회 직전 PID만 일치한다고 안전이 완결됐다고 설명하지 않는다. 파괴적 동작에서 identity를 검증할 수 없으면 보수적으로 거절한다.

**완료 기준:** 명령 없음/권한 거부/hang/잘린 결과, 이미 종료, PID reuse, 다른 프로세스 트리, 조회 직후 Restart를 주입한다. 실패가 성공으로 바뀌거나 Unknown 때문에 임의의 새 프로세스가 시작·종료되지 않아야 한다.

## 11. A09 — App 출력 수집 제거로 방향 변경

**최종 사용자 결정 (2026-09-18):** Port Lens는 로그 저장 도구가 아니다. 각 App의
stdout/stderr와 자체 로그는 해당 App이 관리한다. Port Lens는 자신이 수행한 실행·종료
요청/결과를 기존 `port-lens.log`에 남긴다.

기존 발견 사항은 App 시작 시에만 5MiB 회전 여부를 검사하고 실행 중에는 직접 append해
장기 실행 시 파일이 무제한 증가한다는 것이었다. PR #6의 최초 후보 `12b2f2d`는 별도
수집기로 상한을 구현했으나, 종료 후 잔류 프로세스·계속되는 디스크 쓰기·출력 경로 의존성
때문에 이 접근은 채택하지 않는다. 5MiB × 2개 정책과 수집기 명세는 대체되었다.

**변경 계약:** 자동 stdout/stderr 수집·수집기 진입점·App별 Logs UI/API를 제거하고
새로 실행하는 App의 두 스트림은 null device로 연결한다. 시작/종료/재시작 요청과 결과,
생성 PID, 관찰한 listener/identity 정보, 종료 명령 결과 및 관찰한 exit 정보를 자체
진단 로그에 기록한다. 기존 상태 판정과 소유권 검증을 재사용하며 health check나
정상 종료 판정을 새로 설계하지 않는다. App 자체 로그와 기존 저장 파일은 변경하지 않는다.

**검증:** 다량의 stdout/stderr 출력이 막히거나 파일을 만들지 않는지, launcher 종료 후에도
App 출력이 정상 동작하는지, exit code가 유지되는지, 자체 진단 로그만 회전하는지 확인한다.
Windows 사용자는 Start/Stop/Restart·Port Lens 종료/재연결을 검증한다.
[검증 문서](../testing/managed-lifecycle-diagnostics.md). N01 이후는 계속 보류한다.

## 12. 병합 전 Windows 검증 체크리스트

1. 격리된 테스트 서버 하나가 두 포트를 열도록 한다. 한 포트로 App 등록 후 Port Lens에서 시작하고, 다른 포트의 Kill을 시도한다. 명령은 거절되고 두 listener 모두 살아 있어야 한다. 실제 업무용 서버나 시스템 프로세스로 시험하지 않는다.
2. monitoring-only 외부 테스트 서버의 명시적 Kill은 확인 후 정상 동작해야 한다. 다른 PID가 같은 포트로 바뀌면 기존 선택은 거절되어야 한다.
3. 등록 App 여러 개에서 polling, Start/Stop/Restart를 수행하며 UI/트레이가 계속 반응하는지 확인한다. 일부 probe 응답을 늦추고 Restart해 새 runtime이 사라지지 않는지 확인한다. 정확한 latency 목표는 측정 기준을 정한 후 사용하며 이번 변경으로 특정 ms를 달성했다고 간주하지 않는다.
4. 기존 verified reattach, Stop suppression, monitoring-only 등록을 회귀 검증한다. 생성시각·ancestry 검사를 건너뛰어 테스트를 통과시키지 않는다.
5. compact idle 갱신, hover-visible drag 시 hide, drag-end catch-up, Open 반복 size drift, 작업표시줄 겹침, mixed-DPI 위치 복원을 확인한다. 기존 수용 수준보다 악화되지 않아야 한다.
6. Minimize의 Compact/Tray 분기, X 종료, tray Quit, 재실행 동작을 확인한다. GUI 회귀가 통과하기 전 draft 해제/병합/새 릴리스를 완료했다고 기록하지 않는다.

## 13. 실행 순서와 인수인계

현재 작은 패치는 Windows CI와 위 체크리스트를 확인한 뒤 검토한다. 후속 구현은 A03 scan 신뢰성 → A04 저장 transaction → A05 App operation gate → A06/A07 공통 상태 표현 → A08 남은 process control hardening → A09 출력 제한 순으로 각각 독립 변경을 권장한다. A08 중 Unknown/timeout에 의한 잘못된 제어 가능성은 A03과 함께 조기에 처리해도 된다. 이 순서는 모든 항목을 한 PR에 합치라는 의미가 아니다.

다음 작업자는 STATE.md, BACKLOG.md, 이 문서와 해당 PR의 실제 head/checks부터 읽는다. 이미 고친 A01/A02를 다시 전면 재설계하지 말고, 검수 기준 SHA와 현재 diff를 비교한다. 변경 전후 테스트 로그와 Windows evidence를 각 단계에서 기록한다. bubble drag D1, UDP, HTTP health check를 안정화 작업에 끼워 넣지 않는다.

## 14. 출처

- [검수 기준 repository](https://github.com/teeeeooo/port-lens/tree/320fe90045ef60d021f88ab9646c8e8a952d5559)
- [원본 lifecycle/IPC](https://github.com/teeeeooo/port-lens/blob/320fe90045ef60d021f88ab9646c8e8a952d5559/src-tauri/src/lib.rs)
- [원본 listener scan](https://github.com/teeeeooo/port-lens/blob/320fe90045ef60d021f88ab9646c8e8a952d5559/src-tauri/src/ports.rs)
- [원본 frontend 상태 및 제어](https://github.com/teeeeooo/port-lens/blob/320fe90045ef60d021f88ab9646c8e8a952d5559/src/App.tsx)
- [원본 registry](https://github.com/teeeeooo/port-lens/blob/320fe90045ef60d021f88ab9646c8e8a952d5559/src-tauri/src/registry.rs), [settings](https://github.com/teeeeooo/port-lens/blob/320fe90045ef60d021f88ab9646c8e8a952d5559/src-tauri/src/settings.rs), [process control](https://github.com/teeeeooo/port-lens/blob/320fe90045ef60d021f88ab9646c8e8a952d5559/src-tauri/src/process_control.rs), [diagnostics](https://github.com/teeeeooo/port-lens/blob/320fe90045ef60d021f88ab9646c8e8a952d5559/src-tauri/src/diagnostics.rs)
- [Tauri: Calling Rust / Async Commands](https://v2.tauri.app/develop/calling-rust/): 동기/비동기 command 실행과 UI 작업 분리 근거.
- [Microsoft: PowerShell Common Parameters](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_commonparameters): ErrorAction/ErrorVariable의 일반 계약. 배포 대상 Windows PowerShell의 실제 no-match 동작은 별도 검증한다.
- [Rust: Read::read_to_string](https://doc.rust-lang.org/std/io/trait.Read.html#method.read_to_string): I/O 및 UTF-8 오류를 무시하지 않아야 하는 근거.
