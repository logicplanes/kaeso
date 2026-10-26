# Motion

Quick, decelerating, from the trigger, never bouncing and never in the way. Motion explains where something came from; it never decorates. Durations and curves: `../ai/tokens.md`.

## Durations

| Token                                                | Use                                          |
| ---------------------------------------------------- | -------------------------------------------- |
| `--k-duration-fast`                                  | Hover, press and color changes               |
| `--k-duration-overlay`, `--k-duration-close-overlay` | Menus and popovers opening, closing          |
| `--k-duration-dialog`, `--k-duration-close-dialog`   | Dialogs opening, closing                     |
| `--k-duration-sheet`, `--k-duration-close-sheet`     | Sheets opening, closing                      |
| `--k-duration-toast-in`, `--k-duration-toast-out`    | A toast arriving, leaving                    |
| `--k-duration-indicator`                             | Tab and segment indicators, the switch thumb |
| `--k-duration-disclosure`                            | Collapsibles and sidebar groups              |
| `--k-duration-chart`                                 | Chart values changing                        |
| `--k-duration-check`, `--k-duration-radio`           | The checkmark drawing in, the radio dot      |
| `--k-duration-copied`                                | How long "Copied" stays                      |

## Easing

Things that arrive decelerate (`standard`, `overlay`); things that leave accelerate (`exit`) and take less time. `pulse` is for loading only.

## Loading

Loading keeps one beat of 800ms on the pulse easing. The spinner is three routes on a ring, like the three branches of the mark; it advances a third of a turn per beat. The skeleton breathes over two beats.

- A busy control says so at once (`aria-busy`, clicks ignored); its spinner waits out the 300ms delay, then stays at least 400ms so it never flickers. `useDelayedLoading` times any other indicator the same way.
- A page's own indicator that goes away with the page can wait out the delay in CSS alone: `show-after-delay`.
- A skeleton shows at once: it is the shape of the page, not an indicator.
- With reduced motion the ring holds still and the light passes from route to route by opacity alone; skeletons hold still.

## Rules

- Overlays scale from 0.97 and fade from their trigger; dialogs from 0.98 in the center; sheets slide from their edge.
- A dialog whose content changes stays centered and grows or shrinks to its new size from the size it has, in `--k-duration-dialog` on the `standard` curve; a sheet from the bottom grows upwards. Keep one dialog open through a task, and change its content, rather than closing one dialog and opening another.
- No page-enter animation and no loops but loading's.
- Never `transition: all`; only the properties that change.
- With reduced motion, transforms are removed and only opacity changes remain.
- Input is never blocked by an animation: a closing menu does not delay the next click.
