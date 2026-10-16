# Psychology

People spend hours a day in this interface. Every rule in the library traces back to how people perceive, decide and feel; review every screen against the rules below.

## The rules

1. **One of everything.** One type scale, one spacing scale, one control height per size, one pattern per job. _(Processing fluency)_
2. **Polish is function.** Visual defects are bugs. _(Aesthetic–usability effect)_
3. **Space signals quality.** Generous margins and 48px section gaps; never cram. _(White space and prestige)_
4. **Round what people touch.** Pills for single-line controls, concentric radii for containers. _(Curved-shape preference)_
5. **Space before lines before boxes.** Group by proximity and tone; frame only the surfaces in `elevation.md`. _(Gestalt)_
6. **Big, close targets.** 32px minimum (44 coarse), row-wide hit areas, menus beside triggers, actions at row ends. _(Fitts)_
7. **Few choices at once.** One primary action per view; ≤ 7 items per menu group; advanced options disclosed. _(Hick–Hyman, choice overload)_
8. **Recognition over recall.** Names beside ids, providers beside models, units beside numbers. _(Cognitive load, working memory)_
9. **Color means something.** Monochrome interface; color only for status and data. _(Pre-attentive processing)_
10. **Steps you can see, and no bigger.** State fills ≈ 3–4 L* apart. _(Just-noticeable difference)_
11. **Never color alone.** Every status carries a word or an icon. _(Pre-attentive processing; WCAG 1.4.1)_
12. **Dark mode is designed, not inverted.** `#fafafa` text on black; weights 20 lighter; parity reviewed side by side. _(Halation and irradiation)_
13. **Measured contrast.** Every pair measured; 12px minimum text; tertiary never carries meaning alone. _(Contrast sensitivity, WCAG 2.2, APCA)_
14. **Read along an edge.** Left-aligned labels, right-aligned tabular numbers, prose ≤ 68ch. _(Reading and scanning)_
15. **Answer within 100 ms.** CSS-only hover and press; skeletons under 1 s; progress beyond. _(Response time limits)_
16. **Motion confirms, then stops.** 120–240 ms, decelerating, no overshoot, no page-enter, no loops. _(Motion perception)_
17. **Controls look like controls.** Fill + ring + shadow; non-controls never mimic them. _(Affordance and signifiers)_
18. **One ink button.** Exactly one primary per view. _(Von Restorff)_
19. **Important at the ends.** Most-used navigation first, account and help last, primary action last in a footer. _(Serial position)_
20. **Show progress, never nag.** No setup checklists or numbered step lists; a new project shows its normal screens with their empty states, and a task in several steps is a `StepDialog`. _(Goal-gradient, Zeigarnik)_
21. **Care for the peaks.** The key reveal, the first request and errors get extra space and calm copy. _(Peak–end rule)_
22. **Undo over confirm.** Reversible actions are instant and undoable; irreversible ones explain themselves. _(Self-determination, perceived control)_
23. **Quiet is premium.** Restraint, monochrome, precise type, nothing loud; simplify what looks busy at 50%. _(Luxury cues, visual complexity)_

### Learned in review

24. **Nothing moves when its state changes.** A button that reads “Copy page” keeps its width when it reads “Copied”; loading, hover, selected and open keep the box. Every label variant is reserved in one grid cell. _(Found in review: a shrinking button reads as a glitch and moves the pointer's target.)_
25. **Rings follow the keyboard.** Focus rings appear for keyboard focus only; a clicked control looks clean, never "stuck selected". _(Found in review: a ring left after a click reads as an error state.)_
26. **Words, not symbols.** Empty states, alert dialogs, badges and status labels carry words; a decorative icon or dot beside them adds noise without meaning. _(Found in review: the icon above an empty state and the dot in a badge made screens look cheaper.)_
27. **Every control text inherits the interface family.** No serif fallback ever renders, in portals and native controls included. _(Found in review.)_
