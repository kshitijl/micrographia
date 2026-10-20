# Initial review of the pages (October 2026)

A working file, not permanent: Kshitij's feedback on each page as given, what was done about
it, and decisions still being discussed. Check each item off once it is done and confirmed.
Delete this file once everything here has been settled.

## Decisions

- **Names.** The MD engine is **Brownie** (Robert Brown; the Kodak Brownie camera that put
  photography in everyone's hands). The project is **Micrographia** (Hooke's 1665 book showing
  the public the microscopic world). Done so far: README, site title, every page's top bar.
  Open: rename the `md` crate to `brownie` (and `md-core`/`md-ref` to match)? Not done yet.
- **Priorities.** The original list of questions comes first. That needs the 3D engine next,
  and phase transitions (boiling, melting, freezing, salt and melting point, triple point,
  temperature holding steady during a phase change) must be visible in it. Written into
  `docs/status.md` under Next.

## Pages

### `reversal`: un-stirring the coffee

Feedback:
1. Pick 5 representative cream molecules, trace their paths and draw them, so the viewer can see
   each one literally retrace its steps on the way back.
2. Replace the checkbox ("When reversing, nudge one cream molecule by 2⁻³² of its diameter") with
   buttons: "nudge representatives", "nudge all cream", "nudge all molecules", so the user nudges
   by hand, then reverses time and sees the retracing go wrong.

Done (to confirm):
- [ ] 1. Five traced cream molecules, spread over the blob (one at its centre, four around it at
  60% of its radius). Each draws its outward path in its own colour and its return path as
  smaller white dots on top, so an exact return shows white dots sitting inside the coloured
  ones. Each traced molecule is also marked with a translucent square.
- [ ] 2. The checkbox is gone. Three buttons, "The 5 traced", "All cream" and "Every molecule",
  each nudge by 2⁻³² σ (one GPU dispatch for a whole species). A nudge can be given any time;
  the next reversal and the arrival message say what was nudged. Text in the notes updated.
- Browser test: drives the buttons (run 8 τ, nudge all cream, reverse) and checks that the
  return fails and that both outward and return paths were recorded.

Open questions:
- Paths are recorded once a molecule has moved 0.2 σ since the last point, so they stay light
  even for long runs. They are never trimmed: after a very long run they get dense.
