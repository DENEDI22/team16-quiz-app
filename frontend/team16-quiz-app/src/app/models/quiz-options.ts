export interface CategoryOption {
  value: string;
  label: string;
}

export interface DifficultyOption {
  value: string;
  label: string;
}

export const categoryOptions: CategoryOption[] = [
  { value: 'Sports', label: 'Sport' },
  { value: 'History', label: 'Geschichte' },
  { value: 'Geography', label: 'Geografie' },
  { value: 'Entertainment: Music', label: 'Musik' },
  { value: 'Science: Computers', label: 'Computer' },
  { value: 'Entertainment: Video Games', label: 'Videospiele' },
  { value: 'General Knowledge', label: 'Allgemeinwissen' },
];

export const difficultyOptions: DifficultyOption[] = [
  { value: 'all', label: 'Alle' },
  { value: 'easy', label: 'Einfach' },
  { value: 'medium', label: 'Mittel' },
  { value: 'hard', label: 'Schwer' },
];

export function getCategoryLabel(values: string[]): string {
  const real = values.filter((v) => v !== 'All');
  if (real.length === 0 || real.length === categoryOptions.length) return 'Alle';
  if (real.length === 1) {
    return categoryOptions.find((c) => c.value === real[0])?.label ?? real[0];
  }
  return `${real.length} Kategorien`;
}

export function getDifficultyLabel(value: string): string {
  return difficultyOptions.find((d) => d.value === value)?.label ?? value;
}
