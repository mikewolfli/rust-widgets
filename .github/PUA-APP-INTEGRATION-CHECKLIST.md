# PUA Integration Checklist - go-on App Level

**Date**: 2026-04-02  
**Project**: go-on (Rust agent proxy)  
**Framework**: PUA v3  
**Integration Mode**: App-level rules for agent proxy

---

## ✅ Framework Files Created

### In `.github/` (Developer-facing)
- ✅ `copilot-instructions.md` - Primary entry point (modified, lines 1-150)
- ✅ `pua-instructions.md` - Detailed framework (327 lines)
- ✅ `pua-enforcement-guide.md` - Enforcement rules (378 lines)
- ✅ `PUA-QUICK-REFERENCE.md` - Quick lookup tables
- ✅ `PUA-START-HERE.txt` - Entry point for quick start
- ✅ `PUA-ACTIVATION.md` - Setup guide
- ✅ `GO-ON-PUA-INTEGRATION-SUMMARY.md` - Integration overview (this project)
- ✅ `UNIVERSAL-ACTIVATION-COMPLETE.md` - Universal activation confirmation
- ✅ `pua-check.py` - Python validator script
- ✅ `check-pua-status.sh` - Bash status checker

### In `RULES/` (App-facing - auto-loaded by go-on)
- ✅ `pua.md` - **NEW: Central rule file for agent proxy enforcement**
- ✅ `README.md` - **UPDATED: Added reference to RULES/pua.md**

### In Project Root
- ✅ `GO-ON_PUA_IMPLEMENTATION.md` - **NEW: Rust implementation guide**
- ✅ `README-PUA-UNIVERSAL.md` - Universal guide for any AI tool
- ✅ `PUA-EMBEDDED.md` - Overview of integration
- ✅ `CLAUDE.md` - Universal agent instructions

### Other
- ✅ `.cursor/rules/pua-enforcement.mdc` - Cursor rule file

---

## 🎯 Two-Level Integration Complete

### Level 1: Developer Protection ✅
**When**: Developers ask AI to work on go-on codebase  
**Where**: `.github/copilot-instructions.md` (lines 1-150)  
**How**: Framework auto-loaded by any AI tool  
**Status**: LIVE

### Level 2: Agent Proxy Runtime ✅
**When**: Users request features via go-on agent proxy  
**Where**: `RULES/pua.md` (auto-loaded in config)  
**How**: Runtime validation pipeline (to be coded)  
**Status**: READY FOR IMPLEMENTATION

---

## 📋 Implementation Checklist

### Phase 1: Framework Documentation ✅
- [x] Create RULES/pua.md
- [x] Update RULES/README.md
- [x] Create GO-ON_PUA_IMPLEMENTATION.md (Rust code guide)
- [x] Create GO-ON-PUA-INTEGRATION-SUMMARY.md
- [x] Verify all files present with py checker

### Phase 2: Core Rust Implementation ⏳
- [ ] Create `src/pua.rs` with PuaTracker struct
- [ ] Implement red line detection (3 categories)
- [ ] Implement pressure escalation (L0-L4)
- [ ] Implement quality compass (5-point validation)
- [ ] Implement iceberg pattern scanning
- [ ] Write unit tests for PuaTracker

### Phase 3: Config Integration ⏳
- [ ] Update `src/config.rs` to load RULES/pua.md
- [ ] Add pua_enabled flag to AppConfig
- [ ] Add PUA configuration section to config parsing
- [ ] Support PUA env variables

### Phase 4: Agent Task Integration ⏳
- [ ] Modify agent task handler to use PuaTracker
- [ ] Implement response validation pipeline
- [ ] Add PuaRejection error type
- [ ] Return formatted rejection to agent with guidance

### Phase 5: Logging & Observability ⏳
- [ ] Extend `src/audit.rs` for PUA violation logging
- [ ] Add PUA metrics collection
- [ ] Create observability dashboard hooks
- [ ] Support PUA violation export

### Phase 6: Testing ⏳
- [ ] Unit tests for red line detection
- [ ] Unit tests for pressure escalation
- [ ] Unit tests for quality compass
- [ ] Integration tests for full validation pipeline
- [ ] Fixtures for test agent responses

### Phase 7: Documentation ⏳
- [ ] Update README.md with PUA section
- [ ] Document config.toml PUA options
- [ ] Create PUA troubleshooting guide
- [ ] Add PUA examples to docs

### Phase 8: Deployment ⏳
- [ ] Build and test with `cargo check`
- [ ] Verify with `cargo test`
- [ ] Run clippy for code quality
- [ ] Document breaking changes (none expected)
- [ ] Create release notes

---

## 🔄 How go-on Loads PUA Rules

```
startup
  ↓
Load config.toml
  ↓
config.rs: apply_auto_rules(config_path)
  ↓
Check: RULES/pua.md exists?
  ↓ YES
Load content into AppConfig::pua_rules
  ↓
Log: "PUA enforcement rules loaded from RULES/pua.md"
  ↓
Set: config.pua_enabled = true
  ↓
Ready for agent requests
```

---

## 📊 File Structure (Current)

```
go-on/
├── RULES/
│   ├── pua.md                      ← ✅ Main rule file (420 lines)
│   ├── global.md                   
│   ├── coding.md
│   ├── review.md
│   ├── common.md
│   └── README.md                   ← ✅ Updated with pua.md reference
│
├── .github/
│   ├── copilot-instructions.md     ← ✅ PUA framework (lines 1-150)
│   ├── pua-instructions.md         ← ✅ Detailed rules
│   ├── pua-enforcement-guide.md    ← ✅ Enforcement details
│   ├── PUA-QUICK-REFERENCE.md      ← ✅ Quick lookup
│   ├── GO-ON-PUA-INTEGRATION-SUMMARY.md  ← ✅ This integration
│   ├── UNIVERSAL-ACTIVATION-COMPLETE.md
│   ├── PUA-ACTIVATION.md
│   └── [other PUA files]
│
├── GO-ON_PUA_IMPLEMENTATION.md     ← ✅ Implementation guide
├── README-PUA-UNIVERSAL.md         ← ✅ Universal guide
├── PUA-EMBEDDED.md                 ← ✅ Overview
├── CLAUDE.md                        ← ✅ Universal instructions
└── [project files...]
```

---

## 🚀 Next Steps (In Order)

**IMMEDIATE** (Next session):
1. Read: `GO-ON_PUA_IMPLEMENTATION.md`
2. Create: `src/pua.rs` (copy PuaTracker code)
3. Verify: `cargo check` passes

**SHORT-TERM** (This week):
1. Integrate PuaTracker with AppConfig
2. Modify agent task handler
3. Add validation pipeline
4. Test with sample agent responses

**MEDIUM-TERM** (This sprint):
1. Complete all Phase 2-5 items
2. Write comprehensive tests
3. Add observability hooks
4. Update documentation

**LONG-TERM** (Production):
1. Deploy with PUA enabled
2. Monitor metrics
3. Refine rules based on real agent behavior
4. Optimize pressure escalation thresholds

---

## 🎯 Success Metrics

Once implemented, track these:

- **Quality Score**: Agent responses avg >= 4.5/5
- **First-Time Approval**: >= 85% of requests approved on first try
- **Red Line Violations**: < 5% of all requests
- **Pressure Distribution**: Most requests stay at L0-L1
- **Iceberg Effectiveness**: >= 80% find related issues when scanning

---

## 🔐 Config Example

```toml
# In config.toml after go-on starts with PUA

[pua]
enabled = true
min_quality_score = 0.8
auto_escalate = true
log_violations = true
violation_log = "pua-violations.log"

# Automatically appended from RULES/pua.md:
# [phases.agent]
# principles = [
#   "Load RULES/pua.md enforcement rules",
#   "Apply three red lines to every agent interaction",
#   ...
# ]
```

---

## 🧪 Quick Test Command (Post-Implementation)

```bash
# After Phase 2-3 complete:
cargo build --release

# Run tests
cargo test pua -- --nocapture

# Check for PUA violations in sample responses
./target/release/go-on --validate-pua test_responses.json

# Monitor PUA metrics
./target/release/go-on --pua-stats
```

---

## 📞 Key Contacts (For Reference)

- **PUA Framework**: tanweai/pua GitHub project
- **go-on Implementation**: Files in RULES/pua.md and GO-ON_PUA_IMPLEMENTATION.md
- **Questions**: Refer to GO-ON-PUA-INTEGRATION-SUMMARY.md

---

## 🔴 Critical Path (What Must Be Done)

```
MUST DO:
1. Code src/pua.rs (copy from guide)
2. Update config.rs (load RULES/pua.md)
3. Integrate in agent handler (validate responses)
4. Test with cargo test

CAN SKIP FOR MVP:
- Fancy observability dashboard
- Advanced metrics export
- Multi-strategy methodology router
- Automated L4 desperation mode

SHOULD DO BEFORE PRODUCTION:
- Comprehensive test suite
- Documentation updates
- Real-world testing with agents
- Performance tuning
```

---

## ✨ Expected Behavior (After Implementation)

### Good Agent Response (Approved)
```
User: "Fix the timeout issue"
Agent: "I found the problem: connection cleanup missing timeout
in conn_cleanup.rs line 42. Root cause: protocol doesn't set
timeout on cleanup phase. Prevention: add 30s timeout in
config.toml. Related issues found: 3 similar patterns, fixed all 3.
Tested with invalid connection state: error properly logged."
go-on: ✅ APPROVED (Quality Score: 4.8/5)
```

### Bad Agent Response (Rejected)
```
User: "Fix the timeout issue"
Agent: "Probably a connection pool issue. Try adding more
connections or increasing the timeout."
go-on: ❌ REJECTED
  Reason: RED_LINE_2 (Unverified claim)
  Red lines: "probably" without verification
  Pressure: L1
  Guidance: "Please verify with logs/metrics first. Show which
             connections are stuck and why. Explain root cause."
Agent retries with better approach...
```

---

## 🏆 Final Status

```
🚀 PUA Framework for go-on: COMPLETE

Level 1 (Developer Protection):
  ✅ Embedded in .github/copilot-instructions.md
  ✅ Works for any AI tool
  ✅ Auto-activates on read

Level 2 (Agent Proxy Protection):
  ✅ Rules defined in RULES/pua.md
  ✅ Implementation guide ready in GO-ON_PUA_IMPLEMENTATION.md
  ✅ Ready to code in Rust

Documentation:
  ✅ 10+ reference files created
  ✅ Implementation guide (327 lines)
  ✅ Integration summary (this file)
  ✅ Quick reference cards

Next: Code the Rust implementation (Phase 2)
Time estimate: 4-6 hours for full implementation
```

---

*Last Updated: 2026-04-02*  
*Status: Framework Complete, Implementation Ready*
