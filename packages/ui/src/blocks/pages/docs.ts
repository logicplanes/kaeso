import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  AuthPage: {
    description: "Sign-in and setup pages: no chrome.",
    props: {
      children: "The form.",
      description: "One sentence under the heading.",
      footer: "Content under the form, such as a link to sign up.",
      title: "The page's heading.",
    },
  },
  EmptyPage: {
    description: "Empty, centered in the panel.",
    props: {
      actions: "One primary and at most one secondary action.",
      children: "One sentence that says what to do, and an action.",
      description: "One sentence: what is absent and the next step.",
      header: "The page header's title, description and actions.",
      size: "How much room the empty state takes.",
      title: "What is missing, in a few words.",
    },
  },
  ListPage: {
    description: "Header, toolbar, then the list or table edge to edge; the panel scrolls.",
    props: {
      children: "The list.",
      header: "The page header's title, description and actions.",
      sheet: "A detail Sheet that keeps the list visible.",
      toolbar: "The filters and display options above the list.",
    },
  },
  OverviewPage: {
    description: "Big numbers, period controls, a chart on the panel, a breakdown, an optional right column.",
    props: {
      aside: "A side column, under the content on phones.",
      chart: "The main chart.",
      children: "Content under the chart.",
      controls: "Controls for the period and grouping.",
      header: "The page header's title, description and actions.",
      metrics: "The key numbers at the top.",
    },
  },
} satisfies ComponentDocs;
