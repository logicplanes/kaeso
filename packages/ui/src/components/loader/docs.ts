import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Loader: {
    description:
      'A status region with the spinner and a label: "Saving provider…", or a page or region that is loading. Its place is kept while it waits out `delay`, so nothing moves when it shows.',
    props: {
      delay:
        "Waits before the spinner and its label show, so quick work shows nothing: `true` for the library's 300ms (`useDelayedLoading`), or milliseconds. None by default.",
      hideLabel: "Keeps `label` for assistive technology only, where the page around it already says what is happening.",
      label: 'Shown beside the spinner ("Saving provider…"); without one, "Loading" is announced in the provider\'s language.',
      loading: "Whether the work is underway; true by default. With `delay` and `minDuration`, it times the spinner.",
      minDuration: "Once shown, they stay at least this many milliseconds: 400 with a delay, 0 without.",
      size: "The loader's size on the type scale.",
      tone: "The spinner's and the label's color: `secondary` by default.",
    },
  },
  Spinner: {
    description:
      "Kaeso's spinner: three routes on a ring, the lead in full color and the two behind it fading, advancing a third of a turn per 800ms beat on the pulse easing. Only the svg turns (a composited transform). With reduced motion it holds still and the light passes from route to route instead. The routes are one 84° arc, turned 120° and 240°, with 36° between them, so the gaps stay open at 12px. The turns, the fill and a 1.5px stroke that keeps its width at every size come from kaeso.css. It takes the current color: in a button, the button's.",
    props: {
      label: "Announced to assistive technology; omit when the surrounding control already says it is busy.",
      size: "The spinner's size on the type scale.",
    },
  },
} satisfies ComponentDocs;
