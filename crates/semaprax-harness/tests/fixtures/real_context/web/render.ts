export interface Row {
  label: string;
  cents: number;
}

export function formatCents(cents: number): string {
  return (cents / 100).toFixed(2);
}

export function renderTotal(rows: Row[]): string {
  let total = 0;
  for (const row of rows) {
    total += row.cents;
  }
  return formatCents(total);
}

export class Statement {
  constructor(private rows: Row[]) {}

  summary(): string {
    return renderTotal(this.rows);
  }
}
