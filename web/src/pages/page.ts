export interface Page {
  id: string;
  title: string;
  group: string;
  mount(container: HTMLElement): void;
}
