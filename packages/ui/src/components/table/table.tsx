import { forwardRef, type HTMLAttributes, type TdHTMLAttributes, type ThHTMLAttributes } from "react";
import { cn } from "../../utils/cn";
import { STATUS_TONE_TEXT, type StatusTone } from "../../utils/tone";

export const KAESO_TABLE_VARIANTS = {
  align: {
    start: { classes: "text-left", description: "Text (default)" },
    end: { classes: "text-right tabular", description: "Numbers" },
  },
} as const;
export const KAESO_TABLE_DEFAULT_VARIANTS = { align: "start" } as const;

const Root = forwardRef<HTMLTableElement, HTMLAttributes<HTMLTableElement>>(function Table({ className, ...props }, ref) {
  return <table ref={ref} className={cn("w-full border-collapse text-body", className)} {...props} />;
});
Root.displayName = "Table";
const Head = forwardRef<HTMLTableSectionElement, HTMLAttributes<HTMLTableSectionElement>>(function TableHead(props, ref) {
  return <thead ref={ref} {...props} />;
});
Head.displayName = "Table.Head";
const Body = forwardRef<HTMLTableSectionElement, HTMLAttributes<HTMLTableSectionElement>>(function TableBody(props, ref) {
  return <tbody ref={ref} {...props} />;
});
Body.displayName = "Table.Body";
const Row = forwardRef<HTMLTableRowElement, HTMLAttributes<HTMLTableRowElement>>(function TableRow({ className, ...props }, ref) {
  return <tr ref={ref} className={cn("hairline-b last:shadow-none", className)} {...props} />;
});
Row.displayName = "Table.Row";
const HeaderCell = forwardRef<HTMLTableCellElement, Omit<ThHTMLAttributes<HTMLTableCellElement>, "align"> & { align?: "start" | "end" | undefined }>(
  function TableHeaderCell({ align = "start", className, ...props }, ref) {
    return (
      <th
        ref={ref}
        scope="col"
        className={cn(
          "h-36 px-12 text-caption font-medium whitespace-nowrap text-fg-secondary hairline-b-strong first:pl-0 last:pr-0",
          KAESO_TABLE_VARIANTS.align[align].classes,
          className,
        )}
        {...props}
      />
    );
  },
);
HeaderCell.displayName = "Table.HeaderCell";
export interface TableCellProps extends Omit<TdHTMLAttributes<HTMLTableCellElement>, "align"> {
  align?: "start" | "end" | undefined;
  tone?: StatusTone | undefined;
}
const Cell = forwardRef<HTMLTableCellElement, TableCellProps>(function TableCell({ align = "start", tone, className, ...props }, ref) {
  return (
    <td
      ref={ref}
      className={cn(
        "h-44 px-12 py-12 align-middle text-fg first:pl-0 last:pr-0",
        KAESO_TABLE_VARIANTS.align[align].classes,
        tone && STATUS_TONE_TEXT[tone],
        className,
      )}
      {...props}
    />
  );
});
Cell.displayName = "Table.Cell";

export const Table = Object.assign(Root, { Head, Body, Row, HeaderCell, Cell });
