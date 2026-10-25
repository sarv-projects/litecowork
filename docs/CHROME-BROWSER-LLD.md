# Chrome Browser, Profile Connection, Side Browser and Takeover LLD (V1)

Status: NORMATIVE V1 TARGET, not an assertion of shipped browser integration. Owners E07-S01–S03/S05, E08-S05/S07, E04 trust. Existing ENVIRONMENTS/TRUST/EFFECT/LEASE/NETWORK constraints outrank presentation convenience.

## Engine decision and platform reality

A Tauri WebView is not Google Chrome. Windows WebView2 is Edge Chromium; macOS WKWebView and Linux WebKitGTK are WebKit. A Chrome-grade live browser cannot be promised from an ordinary embedded Tauri WebView. V1 must qualify a real Chromium/Google Chrome execution provider and independently choose a live-view surface with correct browser rendering/interaction. Do not claim a page is "Chrome" when it is only a screenshot or WebView. Browser provider and presentation adapter are separate contracts.

V1 provider availability is a separate value from the user's chosen interaction mode:
`AvailabilityStatus = READY | NEEDS_SETUP | UNAVAILABLE | OFFLINE | UNKNOWN`. `UNAVAILABLE`
is never a browser mode. A user may select MANAGED_CHROMIUM, CONNECT_EXISTING_CHROME,
MANUAL_ONLY, or HEADLESS_TASK only when the corresponding provider and session surface are
qualified; otherwise the UI shows the unavailable status and repair path.

V1 eligible modes:
- MANAGED_CHROMIUM: owned isolated browser data directory, persistent under explicit Workspace/profile policy, Chrome/Chromium/Chrome for Testing chosen from installed qualified distribution. Launch with Playwright or approved CDP adapter. Browser runs outside direct untrusted model access behind control-lease gateway.
- CONNECT_EXISTING_CHROME: consent-based attach to an already running compatible Chrome; only after per-session explicit authorization from Chrome/user, with clear high-trust warning; feature gate by actual Chrome version and API. Chrome 144+ DevTools auto-connect is a candidate, not universal.
- MANUAL_ONLY: user owns actions in browser session; agent can observe only what was explicitly authorized. No hidden automation on sensitive sites.
- HEADLESS_TASK: noninteractive managed browser for authorized background steps; handoff to visible managed browser is a new surface transition only after provider support, not pretend same session.

Chrome 136+ disregards remote debugging switches against its default user directory. Playwright explicitly does not support automating the default Chrome user profile. Therefore do not point Playwright at the normal profile or copy its encrypted cookies. Managed profiles are separate, user-visible, data-minimized and permission-controlled. Existing-profile import is limited to supported user-authorized exports/preferences/bookmarks. Existing signed-in session reuse requires a separate explicit Chrome connection and warning, never stealth cookie extraction.

## User journeys

1. Open Workbench > Browser; choose Managed browser, Connect existing Chrome or Manual browsing. Show qualified Chromium/Chrome brand, version, profile identity, connection status and authorized scopes. No browser operation from plain panel open without a real provider.
2. First managed launch: Choose existing LiteCowork browser profile or Create isolated profile, set retention and download directory, review Workspace/trust/host constraints, launch; next visits may reopen profile if no competing process holds it.
3. Import wizard: choose source via supported Chrome export or OS picker (bookmarks/settings only); preview counts/content types, duplicates and destination; dry-run and explicit apply; show partial failure/retry; no secrets/history/password autofill import by default; never claim login imported.
4. Connect existing Chrome: explicit user permission and Chrome confirmation; disclose potential access to open tabs, session storage and authenticated sites; refuse if version/consent/integrity unavailable; allow revoke/detach at any time.
5. Browser navigation: address field, back/forward/reload, tab list, current tab, new tab, close tab, open in external Chrome, download status, zoom, find, site permissions, security status, copy URL; keyboard and accessible labels.
6. Task asks for web action: requires Environment/Workspace/Task/Attempt authorization, site grants, capability grant, fresh execution lease and Effect policy; show real action in activity, not phantom cursor animation.
7. User presses Take control: atomic epoch transfer AGENT -> USER; pending agent inputs cancelled/discarded; user retains focus; provider observes user modifications only after fresh capture.
8. User presses Return to agent: explicit confirmation, revalidate page/browser/target auth/Task/lease; increment epoch, capture DOM/a11y/screenshot state and form edit status; agent replans from fresh state, never replays stale selectors.
9. User closes side pane: does NOT necessarily close browser or cancel Task. Close tab/browser/profile and Stop Task are separate actions with explicit impact dialogs.
10. Resume after restart: retained profile reopened only after owner approval and profile lock/health check; precrash navigation and writes marked unknown until reconciled; never infer action succeeded because URL matches.

## Structural HLD

~~~text
React Workbench BrowserSurface
  <- versioned read-only BrowserProjection/BrowserFrame
  -> BrowserControlController (user commands)
Tauri BrowserBridge (authenticated local command/stream, NO CDP in webview)
  -> litecoworkd BrowserSessionService
      -> BrowserSessionStore (durable non-secret scope, tab and identity metadata)
      -> BrowserProviderRegistry (OS/version/capability-qualified)
      -> BrowserLeaseArbiter (single actor, epoch fencing)
      -> BrowserActionService (approval, trust, effect/outbox, exact requestId)
      -> BrowserObservationService (DOM/a11y/frame capture, redaction, size caps)
      -> Managed Chromium provider OR explicit existing Chrome provider
      -> Output/Download/Artifact binder
~~~

Provider adapters never grant themselves authority. BrowserSession is a Runtime/Environment-local provider handle; Task/Attempt/Effect remain Core authorities. Chrome sessions cannot migrate to cloud by relabeling location. Local daemon uptime cannot guarantee GUI access while OS locked/asleep.

## Target data and event contracts (not yet promoted routes)

~~~ts
type BrowserMode = "MANAGED_CHROMIUM"|"CONNECT_EXISTING_CHROME"|"MANUAL_ONLY"|"HEADLESS_TASK";
type BrowserControlOwner = "AGENT"|"USER"|"NONE"|"TRANSFER_PENDING";
type BrowserSessionProjection = {
  sessionId:string; environmentId:string; runtimeIncarnationId:string;
  workspaceId:string; taskId?:string; attemptId?:string;
  mode:BrowserMode; provider:string; browserVersion:string;
  profileRef:string; // opaque, not filesystem paths or secrets
  tabs: Array<{tabId:string; title:string; url:string; loading:boolean; blocked:boolean}>;
  activeTabId:string|null; owner:BrowserControlOwner; controlEpoch:number;
  health:"STARTING"|"READY"|"DEGRADED"|"DISCONNECTED"|"RECOVERING"|"CLOSED";
  viewMode:"LIVE_INTERACTIVE"|"OBSERVATION_FRAMES"|"EXTERNAL_WINDOW"|"UNAVAILABLE";
  observationRevision:number; lastObservedAt:string|null;
};
type BrowserCommand = {
  requestId:string; expectedEpoch:number; expectedSessionRevision:number;
  sessionId:string; tabId?:string; action:
    "OPEN_TAB"|"CLOSE_TAB"|"NAVIGATE"|"BACK"|"FORWARD"|"RELOAD"|
    "TAKE_CONTROL"|"RETURN_CONTROL"|"DETACH"|"CLOSE_SESSION"|"CAPTURE"|
    "FIND"|"SET_ZOOM"|"OPEN_EXTERNAL"|"APPROVE_DOWNLOAD";
  arguments:unknown;
  actor:"USER"|"AGENT"; // caller intent only, NEVER authority: service derives actual actor from authenticated principal + control lease
};
type BrowserActionReceipt = {
  requestId:string; state:"REJECTED"|"ADMITTED"|"IN_PROGRESS"|"APPLIED"|"AMBIGUOUS"|"BLOCKED";
  sessionRevision:number; controlEpoch:number; effectId?:string; retryAllowed:boolean;
};
~~~

Never put credentials, cookies, unredacted HTML, CDP secrets, full profile path or storage tokens in domain events/UI logs. Browser event registry target groups: browser.session.observed, browser.tab.changed, browser.control.requested/committed, browser.action.admitted/completed/ambiguous, browser.observation.available, browser.download.review_required/recorded, browser.disconnected/recovered. Promote each with exact schema/ACL/payload to EVENTS and operator OpenAPI before dispatch. Events are immutable facts, not user-action APIs.

## Required service functions (pseudocode/contract)

| Function | Authority, algorithm and result |
|---|---|
| discoverBrowserProviders(os, installed) | Inspect supported Chrome/Chromium distributions, version and CDP/Playwright adapter qualification; no guessing from binary alone; return statuses |
| createManagedProfile(workspace, policy, reqId) | Verify owner + storage quota; allocate fresh isolated directory with OS protection; save opaque ref; idempotent journal; never copy auth material |
| previewProfileImport(export, target) | Parse allowlisted non-secret bookmark/config files, validate limits/URLs and duplicates; return review diff |
| commitProfileImport(previewDigest, approval, reqId) | Enforce same file digest/target/version; atomic import or recorded partial entries; no credential copying |
| attachExistingChrome(consent, capabilities) | Check browser version, Chrome consent, origin and attach lifetime; bind only authorized session; fail closed on unexpected profile |
| openBrowserSession(request) | Verify Workspace, Environment, profile lock, provider readiness, retention and runtime incarnation; return session with initial tab |
| listBrowserTabs(session, cursor) | Authorized bounded listing; safe title/url fields; stale/removed tab tombstones |
| performBrowserAction(command) | Validate actor, epoch, tab generation, preconditions, effect/grants; record intent/outbox then dispatch; response-loss => reconcile before retry |
| requestHumanTakeover(session, expectedEpoch) | Fence current agent input, wait bounded in-flight safe point, increment epoch, transfer input authority to user |
| returnToAgent(session, expectedEpoch) | Confirm user, refresh observation, test grants/leases and compatibility, transfer in new epoch; stale actions never replay |
| captureBrowserObservation(session, scope) | DOM/a11y/screenshot only with authorization; redact secrets; bounded frame size, privacy zones and source provenance |
| reconcileBrowserAction(requestId) | Correlate provider state/Effect receipt; UNKNOWN remains ambiguous; do not double submit form |
| detachBrowserSession(session) | Stop input and browser bridge; avoid forcibly killing a user-owned Chrome profile; revoke session grant |
| closeManagedBrowser(session) | Fence inputs, flush approved downloads, process-quiescence and profile lock cleanup; preserve retained profile if policy |
| recoverBrowserSession(incarnation) | Never trust stale provider handle; rebind only after OS-level observation and owner scope; revalidate all leases |
| openBrowserInWorkbench(session, tab) | Select exact session/tab without creating authority; show true live/frames/external label |
| exportBrowserArtifact(download) | Verify immutable file bytes/media, sandbox and policy; quarantine unsupported downloads; tie ArtifactVersion to provenance |

## Live view fidelity decision gate

Option A: qualified native Chromium embedding/remote surface that renders actual frames and forwards permitted pointer/key input. Option B: real external Chrome window plus synchronized observation in right pane. Option C: screenshots/DOM snapshots read-only. The UI must identify which option is active. A fake iframe in Tauri is unacceptable as Chrome parity. Requirement to implement an actual interactive side-browser is NOT automatically satisfied by CDP automation; E07 SYSTEM+USER gate needs real video/frame correctness, click coordinate mapping (DPI, zoom, scroll, multiscreen), IME, focus, popup handling, file dialog, drag/drop and resizing on each supported OS. If no qualified embedding, use explicit external Chrome and label it accurately; never silently degrade.

## Page behavior and control conflicts

- Pending action or spinner must be keyed to one operation; simultaneous user/agent input blocked by epoch even if old HTTP calls arrive.
- Navigation may alter origin and authorization; recheck before every consequential action. Popups/new tabs are untrusted until associated with permitted session. Redirect to a new origin is not proof of user grant.
- Form submission/retry is Effect-class operation. Duplicate button clicks, network ambiguity and timeout require explicit reconciliation.
- User edits form during takeover; invalidate previous DOM and selector snapshots. Return does not automatically submit.
- Password fields, payments, identity prompts, CAPTCHA, bot protection, browser security warnings, permissions and MFA require human handling; no bypass promise.
- Download, upload and clipboard have independent permission checks; never auto-share desktop clipboard or profile contents to model.
- Revoke/expired permission mid-action must fence new effects, record ambiguity for already external sends and require user intervention as needed.
- Browser crash, profile busy, remote debugger detachment, OS lock, GPU/render failure, display resize, offline, tab close, cross-Workspace switch and app-close are honest state transitions.
- Cache browser frames with TTL and identity. No old screenshot shown as live after reconnection; stale frame labelled.
- Browsing history/URLs may be private; show only authorized scope, mask sensitive query parameters and keep diagnostic logging minimal.

## Settings surface

Settings > Browser & Computer: Provider status; Chrome path/channel/version; Managed profiles with Workspace assignment; Import bookmarks/preferences; Connect existing session (when eligible); Per-profile retention, download folder, permissions and data deletion; session detach; site grants; observation and screenshot permission; activity diagnostics; storage size; cleanup preview. All change controls require exact expected versions and user acknowledgments for irreversible actions. No general "import everything including login" toggle. Distinguish provider not installed vs unsupported OS vs consent required vs currently disconnected.

## Test matrix and release evidence

Real Chrome/browser tests must cover profile creation, return visits, managed profile contention, import duplicate/bad export/no secret import, Chrome 136 restriction, current Chrome explicit connect, tab/popup/redirect, downloads, agent/user lease race, stale screenshot, mis-scaled coordinates, zoom/high DPI, a11y, login manual handoff, cancellation, crash/restart, double-submit reconciliation, multi-Workspace isolation, offline/locked screens, cleanup. Log exact browser/OS/provider versions and sanitized evidence. Static TypeScript tests do not qualify a working browser.

Official sources to recheck/pin:
- https://playwright.dev/docs/api/class-browsertype
- https://developer.chrome.com/blog/remote-debugging-port
- https://developer.chrome.com/blog/chrome-devtools-mcp-debug-your-browser-session
- https://developer.chrome.com/docs/devtools/agents/get-started/configuration
- https://chromedevtools.github.io/devtools-protocol/tot/Target/
- https://v2.tauri.app/reference/webview-versions/


## Chosen V1 side-browser implementation path and required spike

**Baseline provider**: controlled Google Chrome/Chromium (prefer Chrome for Testing only for managed automation profiles), Playwright for resilient DOM/a11y/locator work, CDP for versioned browser tab/viewport/observation operations. Implement a Chrome-rendered side-panel **screencast bridge** as first candidate, not an iframe/WebView claiming to be Chrome. If real interactive frame/control tests fail a supported OS, expose a first-class Open in real Chrome external-window mode with explicit limitation; do not market it as embedded Chrome on that OS.

~~~text
Chrome Browser process
  -> CDP Target attach (qualified version) -> Page.startScreencast
  -> Page.screencastFrame(sessionId, metadata, compressedPixels)
  -> bounded frame queue; acknowledge via Page.screencastFrameAck
  -> authenticated local Tauri stream -> Workbench BrowserCanvas
  -> UI pointer/keyboard/scroll coordinate transform + controlEpoch
  -> authorized Tauri command -> browser lease gateway
  -> CDP Input.dispatchMouseEvent / dispatchKeyEvent / insertText
  -> fresh observation/reconciliation
~~~

CDP screencast is experimental and may be lower fidelity than native embedding: test frame freshness, max in-flight frames, visual stutter, iframe/popup quirks, IME, clipboard, file chooser, fullscreen, focus and site permission prompts. Never claim that browser chrome controls (password manager, extension toolbar, native permission dialogs) render inside a tab-only screencast; those require external Chrome takeover or a separately qualified native embedding. A user clicking the screen does not grant an agent permanent browser control.

Required functions and outputs:
- startChromeScreencast(session, tab, viewportPx, DPR, maxFramesInFlight) -> stream binding with frame sequence and lifetime.
- transformPointerCoordinates(clientXY, paneBounds, frameMetadata, viewportScale) -> CSS viewport x/y or reject stale geometry.
- forwardHumanBrowserInput(input, expectedControlEpoch, tabGeneration) -> acknowledged queued input only for USER actor.
- acknowledgeBrowserFrame(frameSessionId, frameId) -> bounded backpressure; discard old frames before decode on owner/route change.
- stopChromeScreencast(session) -> close capture stream, wipe buffered frames and revoke view subscription; do not terminate unrelated Chrome tabs.
- registerBrowserPopup(tabCreation, opener, sourceOrigin) -> quarantined tab until ownership/origin policy checked.
- resolveNativeBrowserDialog(dialogType, userAction) -> show actual Chrome window or explicitly supported platform dialog; never synthesize successful acceptance.

Screencast metadata must include BrowserSession id, tab generation, control epoch, capture time, viewport CSS size, deviceScaleFactor, scroll offset/transform revision, frame sequence, content type and digest. Expired/wrong-session frame is rejected; no screenshots to arbitrary logs or persisted chat without explicit source pin. Browser keyboard shortcut handling distinguishes LiteCowork global shortcuts from focused Chrome content; Ctrl+L targets side-panel address bar, not text in browser page. Browser rendering exceptions never run JS supplied by web page in the LiteCowork WebView.

Recommended official protocol references:
- https://chromedevtools.github.io/devtools-protocol/tot/Page/ (Page.startScreencast, screencastFrame, screencastFrameAck)
- https://chromedevtools.github.io/devtools-protocol/tot/Input/ (dispatchMouseEvent, dispatchKeyEvent, insertText)
- https://chromedevtools.github.io/devtools-protocol/tot/Target/ (target/session discovery)
All protocol methods must be tested against the pinned Chrome binary because CDP includes experimental fields and methods.

## Canonical transport and route qualification note (2026-10-12 audit)

Browser commands above use `requestId` as a logical idempotency handle in pseudocode. When exposed through the Operator, use the existing `Idempotency-Key` header and map it to domain RequestId; the local IPC `request_id` is only request/response correlation. Also enforce `If-Match` and `X-Workspace-ID` per docs/API.md. Browser route paths in V1-FUNCTION-FILE-CATALOG.md are **candidates**, not existing Operator endpoints: reconcile with current API/OpenAPI, assign a single owner and implement every handler, ACL, event and migration before presenting browser controls as working.
