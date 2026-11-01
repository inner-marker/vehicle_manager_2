# Voice Capture Plan (Draft v2)

Sub-plan for hands-free logging in the vehicle maintenance app. Lets a user log a fuel-up, meter reading, or squawk by voice, with no typing while the vehicle is moving.

---

## 1. Principles

- **Opt-in, never intrusive.** Voice is a tool the user chooses. It is off until enabled in settings, and the app never switches into voice mode on its own.
- **Driving detection only suggests.** When driving is detected, the app may show a small "Voice log" chip or mic button. Nothing opens or speaks until the user taps it.
- **Offline first.** All speech recognition and speech output run on the device.
- **Rust core, thin native shims.** Dialog logic, parsing, and data handling live in Rust.
- **Drafts only.** Voice entries are never signed records. They are saved as drafts and reviewed, edited, and signed later through the normal flow (keeps the AC 120-78B-style signature process intact).

## 2. Non-goals (for now)

- CarPlay / Android Auto screens.
- Free-form natural-language parsing ("I just put 18 gallons in at 42k").
- Siri / Assistant integration (see Phase 6).
- Wake word / always-listening.

---

## 3. User experience

### Starting voice mode
- Settings: "Voice logging" (off by default).
- When enabled:
  - A mic button is always available on the main screen.
  - If driving is detected, a small "Voice log" chip appears as a suggestion. Ignoring it does nothing.
- Tapping either opens the voice screen.

### Voice screen
- One large microphone button. No text fields, lists, or scrolling.
- Available commands shown as short quoted words, readable at a glance:
  - **"Fuel up"**  **"Squawk"**  **"Reading"**  **"Options"**
- Saying **"Options"** reads the list aloud (phone-menu style, on request only).

### Conversation style
Guided, one question at a time. Vehicle is always asked first, because every later question depends on that vehicle's field setup (Section 6).

```
User:  "Fuel up"
App:   "Which vehicle?"
User:  "Truck"
App:   "Odometer? Last was 42,180."
User:  "42,300"
App:   "Gallons?"
User:  "18.4"
App:   "Price per gallon? Say skip to leave blank."
User:  "Skip"
App:   "Truck, 42,300 miles, 18.4 gallons. Save it?"
User:  "Yes"
App:   "Saved as draft."
```

### Global words (work at any prompt)
| Word | Action |
|---|---|
| "Cancel" | Discard the entry, return to idle |
| "Repeat" | Re-read the current question |
| "Go back" | Return to the previous field |
| "Skip" | Leave an optional field blank |
| "Options" | Read available commands or choices |
| "Yes" (at a suggested value) | Accept the suggested value (e.g. last odometer) |

### Vocabulary rules
- Commands are 1–2 words, chosen to sound very different from each other (avoid pairs like "fuel" / "full").
- Top-level list stays short (about 3–5 commands).
- Each field's spoken prompt matches its label in the app, so what the user hears matches what they see when reviewing.

---

## 4. Drafts on the dashboard

- The app dashboard shows a **Voice drafts** list of recent open drafts entered by voice.
- Each item shows: vehicle, event type, time logged, and who logged it.
- Expanding an item shows the captured fields and the **raw transcript**, so the user can check what was heard (especially for squawks).
- Actions per draft: **Edit**, **Finalize** (goes into the normal sign-off flow), **Discard**.
- Drafts sync like any other local data.

---

## 5. Driving detection

Detection only controls whether the "Voice log" suggestion chip appears. It never opens voice mode. A manual mic button always exists when voice logging is enabled.

| Platform | Signal | API |
|---|---|---|
| iOS | Car audio connected (CarPlay or car Bluetooth) | `AVAudioSession` route, port type `.carAudio` / Bluetooth |
| iOS | Phone thinks it is in a vehicle | Core Motion `CMMotionActivityManager` (`automotive`) |
| Android | Android Auto connected | Car App Library `CarConnection` |
| Android | In-vehicle activity | Google Activity Recognition (`IN_VEHICLE`) |
| Desktop | — | None; mic button only |

Notes:
- iOS does not tell apps that Driving Focus is on; use the signals above.
- Permissions: `NSMotionUsageDescription` (iOS), activity recognition (Android). Only requested when the user enables voice logging.
- Native shims report a simple `DrivingState` to Rust; Rust decides whether to show the chip.

---

## 6. Per-vehicle fields

Different vehicles and assets need different fields for the same event type (e.g. the truck tracks key-on, engine, and idle hours; an aircraft might track Hobbs and tach time).

- Each vehicle has a field configuration per event type: field name, label, type, unit, required/optional, and a sensible range.
- The **same definition** drives the regular forms, the voice flows, and the review screen. Set up once in the vehicle's settings.
- Voice flows are **built at runtime** from that vehicle's configuration, not hard-coded scripts:
  - Required fields are always asked.
  - Optional fields are asked with "say skip," or can be left for review (setting per field, open question).
- Units are spoken in prompts and readbacks ("Engine hours?", "18.4 gallons").
- Ranges drive sanity warnings (Section 7).

---

## 7. Dialog engine (Rust core)

A small state machine, shared by all platforms and testable without audio.

- Text in, text out. Audio is only a wrapper, so every flow can be driven by typed input and unit tests.
- Flow = command + event type + prompts built from the selected vehicle's field configuration.
- Interrupted flows **pause** instead of being thrown away. The next tap on the mic offers to resume ("Continue the fuel up for the truck?").
- Each saved draft records the logged-in user and `source = voice`.

### Sketch

```rust
/// One field definition from a vehicle's configuration.
/// Shared by forms, voice flows, and the review screen.
pub struct FieldDef {
    /// Column / key in the event detail table.
    pub key: String,
    /// Label shown in the app; also used as the spoken prompt.
    pub label: String,
    /// What kind of value this is (number, text, choice, ...).
    pub kind: FieldKind,
    /// Spoken and displayed unit, e.g. "gallons", "hours".
    pub unit: Option<String>,
    /// If false, the user may say "skip".
    pub required: bool,
    /// Expected range; values outside it trigger a spoken warning.
    pub range: Option<std::ops::RangeInclusive<f64>>,
}

/// A voice flow built for one vehicle at the moment the user picks it.
pub struct Flow {
    /// Event type this flow creates as a draft.
    pub event_type: EventType,
    /// Vehicle the entry belongs to; chosen first.
    pub vehicle_id: VehicleId,
    /// Questions asked in order, built from that vehicle's FieldDefs.
    pub prompts: Vec<FieldDef>,
}

/// Where the conversation currently is.
pub enum DialogState {
    /// Waiting for a top-level command ("fuel up", "squawk", ...).
    Idle,
    /// Command given; asking which vehicle before building the flow.
    ChoosingVehicle { event_type: EventType },
    /// Asking prompt `index` of the active flow.
    Asking { flow: Flow, index: usize, answers: Answers },
    /// All fields filled; reading back and waiting for yes/no.
    Confirming { flow: Flow, answers: Answers },
    /// Interrupted mid-flow; kept so the user can resume.
    Paused { flow: Flow, index: usize, answers: Answers },
}

impl DialogEngine {
    /// Feed one recognized utterance in; get the next thing to say back.
    pub fn handle(&mut self, utterance: &str) -> Reply {
        // 1. Normalize text (lowercase, trim, strip punctuation).
        // 2. Check global words ("cancel", "repeat", "go back", "skip", "options").
        // 3. If a suggested value is pending, accept "yes" as that value.
        // 4. Otherwise parse against the current FieldDef (kind, unit, range).
        // 5. On parse failure, re-ask with a short hint; after two failures
        //    offer "skip" (optional fields) or "cancel".
        todo!()
    }
}
```

### Parsing and checks
- Words-to-numbers ("forty two thousand three hundred" → 42300); Whisper usually outputs digits, but handle both.
- Fuzzy-match short answers (vehicle names, yes/no, meter names) against known choices.
- Sanity checks give a spoken warning, never a hard block:
  - Odometer / meter lower than last recorded reading.
  - Unusually large jump since last reading.
  - Value outside the field's configured range (e.g. gallons over tank size).

---

## 8. Initial flows

All flows ask for the vehicle first, then that vehicle's configured fields.

| Command | Typical prompts | Creates |
|---|---|---|
| "Fuel up" | vehicle, odometer, fuel quantity, price (optional), plus any vehicle-specific fields | Fuel event (draft) |
| "Reading" | vehicle, which meter (from that vehicle's meters), value | Meter reading (draft) |
| "Squawk" | vehicle, odometer, describe the issue (free dictation), plus any vehicle-specific fields | Squawk (draft) |

Notes:
- **Odometer in squawks:** a quick glance at the dash is safe, so squawks ask for it. The prompt suggests the last known reading; the user says "yes" to accept or reads the real number.
- Squawk description is saved as the Whisper transcript. Optionally keep the audio clip for review (setting, off by default).

---

## 9. Speech pipeline

### Speech to text: `whisper-rs` (whisper.cpp)
- Fully on-device, MIT licensed, no network.
- Start with the **base** model; test **small** if accuracy is weak.
- Initial prompt built from app vocabulary: command words, vehicle names, field labels, common part names.
- Push-to-talk capture with a short silence timeout to end each answer.
- Model is a downloadable asset (downloaded when voice logging is enabled), not baked into the app binary.

### Text to speech: `tts` crate
- Wraps each platform's built-in voice (AVSpeechSynthesizer, Android TTS, SAPI, Speech Dispatcher).
- Duck or pause other audio while speaking, where the platform allows.

### Audio capture
- `cpal` for microphone input where possible; native shim as fallback.
- Permissions: `NSMicrophoneUsageDescription` (iOS), `RECORD_AUDIO` (Android), requested only when voice logging is enabled.

---

## 10. Safety

- One tap to start; everything after is voice.
- No text fields, lists, or scrolling on the voice screen.
- Silence timeout pauses the flow (resumable) rather than discarding it.
- Short spoken prompts; long read-outs only on "Options".
- Only glance-level inputs while driving (reading a gauge on the dash is fine; nothing that needs reading the phone).

---

## 11. Testing

- **Phase 1 text mode:** drive every flow with typed input and unit tests.
- **Replay test mode:** feed recorded answers through the full pipeline to tune Whisper without driving.
- **Real-world noise:** record test answers in the actual vehicle with windows down, road noise, and radio on. This will be the biggest accuracy factor.

---

## 12. Phases

| Phase | Deliverable |
|---|---|
| 1 | Per-vehicle field definitions + dialog engine in Rust core, driven by **typed text** on desktop; full unit tests |
| 2 | Voice drafts list on the dashboard (edit / finalize / discard) |
| 3 | `whisper-rs` + `tts` on desktop; push-to-talk; model download; replay test mode |
| 4 | Audio pipeline on iOS and Android (permissions, model storage) |
| 5 | Driving detection shims; "Voice log" suggestion chip (opt-in only) |
| 6 (later) | Siri App Intents (Swift shim calling Rust via UniFFI) and Android App Actions (deep link via `shortcuts.xml`) |

---

## 13. Ideas to consider

- Spoken fuel economy after a fuel-up ("About 21 miles per gallon").
- Role rules for voice: which roles can create squawks or other event types by voice.
- Optional spoken reminder of open drafts when the trip ends (must respect the opt-in principle; off by default).

---

## 14. Open questions

- Push-to-talk per answer, or one tap then auto-listen for the whole flow?
- Default Whisper model size, and should it be user-selectable?
- Keep squawk audio clips at all, and for how long?
- Should "Squawk" ask for severity (e.g. "ground it" vs "monitor"), or leave that for review?
- With multiple vehicles: always ask, or suggest last-used with a quick "yes"?
- Optional fields: ask with "say skip," or leave for review? Global setting or per field?
- How long do paused flows stay resumable?
- Should the "Voice log" chip appear at all on desktop/web, or mobile only?
