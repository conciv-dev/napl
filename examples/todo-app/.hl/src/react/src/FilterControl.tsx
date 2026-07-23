import type { Filter } from "./types";

type FilterControlProps = {
  filter: Filter;
  onFilterChange: (filter: Filter) => void;
};

const FILTERS: { value: Filter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "active", label: "Active" },
  { value: "completed", label: "Completed" },
];

export function FilterControl({ filter, onFilterChange }: FilterControlProps) {
  return (
    <div className="filter-control">
      {FILTERS.map(({ value, label }) => (
        <button
          key={value}
          type="button"
          className={value === filter ? "filter-button active" : "filter-button"}
          onClick={() => onFilterChange(value)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}
