/**
 * État d'interface du journal de bord : entrées chargées par pages,
 * ajout manuel, intervalle des relevés automatiques et export CSV.
 * Orchestration seulement : les entrées sont écrites par le moteur.
 */
import { LOG_PAGE, type LogbookApi, type LogEntry } from './services/logbook';

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export class LogbookController {
  entries = $state.raw<LogEntry[]>([]);
  autoIntervalMin = $state<number | null>(null);
  /** D'autres entrées plus anciennes peuvent être chargées. */
  more = $state(false);
  error = $state<string | null>(null);
  notice = $state<string | null>(null);
  busy = $state(false);

  constructor(private readonly api: LogbookApi) {}

  private async run<T>(action: () => Promise<T>): Promise<T | undefined> {
    this.busy = true;
    this.error = null;
    try {
      return await action();
    } catch (e) {
      this.error = message(e);
      return undefined;
    } finally {
      this.busy = false;
    }
  }

  /** Recharge la première page et les réglages. */
  async refresh(): Promise<void> {
    await this.run(async () => {
      const [entries, settings] = await Promise.all([this.api.list(), this.api.settings()]);
      this.entries = entries;
      this.more = entries.length === LOG_PAGE;
      this.autoIntervalMin = settings.autoIntervalMin;
    });
  }

  async loadMore(): Promise<void> {
    const last = this.entries.at(-1);
    if (!last) return;
    const older = await this.run(() => this.api.list(LOG_PAGE, last.at));
    if (!older) return;
    const known = this.entries.map((e) => e.id);
    this.entries = [...this.entries, ...older.filter((e) => !known.includes(e.id))];
    this.more = older.length === LOG_PAGE;
  }

  /** Ajoute une entrée manuelle ; `true` si enregistrée. */
  async add(text: string): Promise<boolean> {
    this.notice = null;
    const entry = await this.run(() => this.api.add(text));
    if (!entry) return false;
    this.entries = [entry, ...this.entries];
    return true;
  }

  async remove(id: string): Promise<void> {
    await this.run(() => this.api.delete(id));
    if (this.error === null) this.entries = this.entries.filter((e) => e.id !== id);
  }

  async setInterval(minutes: number): Promise<void> {
    const settings = await this.run(() => this.api.setSettings({ autoIntervalMin: minutes }));
    if (settings) this.autoIntervalMin = settings.autoIntervalMin;
  }

  async exportCsv(): Promise<void> {
    this.notice = null;
    if (await this.run(() => this.api.exportCsv())) this.notice = 'Journal exporté';
  }
}
