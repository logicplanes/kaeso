import { createContext, forwardRef, useContext, type HTMLAttributes, type TdHTMLAttributes, type ThHTMLAttributes } from "react";
import { cn } from "../../utils/cn.js";
import { STATUS_TONE_TEXT, type StatusTone } from "../../utils/tone.js";

export const KAESO_TABLE_VARIANTS = {
  align: {
    start: { classes: "text-left", description: "Text (default)" },
    end: { classes: "text-right tabular", description: "Numbers" },
  },
  density: {
    default: { classes: "px-12", description: "12px between columns (default)" },
    compact: { classes: "px-8", description: "8px between columns, for reference tables of five or more columns" },
  },
} as const;
export const KAESO_TABLE_DEFAULT_VARIANTS = { align: "start", density: "default" } as const;

type Density = keyof typeof KAESO_TABLE_VARIANTS.density;
const DensityContext = createContext<Density>("default");

const LAST = { start: "last:pr-2", end: "last:pr-0" } as const;

export interface TableProps extends HTMLAttributes<HTMLTableElement> {
  density?: Density | undefined;
}
const Root = forwardRef<HTMLTableElement, TableProps>(function Table({ density = "default", className, ...props }, ref) {
  return (
    <DensityContext.Provider value={density}>
      <table ref={ref} className={cn("w-full border-collapse text-body", className)} {...props} />
    </DensityContext.Provider>
  );
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
const HeaderCell = forwardRef<
  HTMLTableCellElement,
  Omit<ThHTMLAttributes<HTMLTableCellElement>, "align"> & { align?: "start" | "end" | undefined; wrap?: boolean | undefined }
>(function TableHeaderCell({ align = "start", wrap = false, className, ...props }, ref) {
  const density = useContext(DensityContext);
  return (
    <th
      ref={ref}
      scope="col"
      className={cn(
        "h-36 text-caption font-medium text-fg-secondary hairline-b-strong first:pl-0",
        KAESO_TABLE_VARIANTS.density[density].classes,
        wrap ? "py-8 align-bottom text-balance" : "whitespace-nowrap",
        LAST[align],
        KAESO_TABLE_VARIANTS.align[align].classes,
        className,
      )}
      {...props}
    />
  );
});
HeaderCell.displayName = "Table.HeaderCell";
export interface TableCellProps extends Omit<TdHTMLAttributes<HTMLTableCellElement>, "align"> {
  align?: "start" | "end" | undefined;
  tone?: StatusTone | undefined;
}
const Cell = forwardRef<HTMLTableCellElement, TableCellProps>(function TableCell({ align = "start", tone, className, ...props }, ref) {
  const density = useContext(DensityContext);
  return (
    <td
      ref={ref}
      className={cn(
        "h-44 py-12 align-middle text-fg first:pl-0",
        KAESO_TABLE_VARIANTS.density[density].classes,
        LAST[align],
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
