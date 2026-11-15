/**
 * État d'interface du journal de bord : entrées chargées par pages,
 * ajout manuel, intervalle des relevés automatiques et export CSV.
 * Les événements d'alarme (historique du moteur) sont intercalés par
 * date, en lecture seule.
 * Orchestration seulement : les entrées sont écrites par le moteur.
 */
import type { AlarmEvent } from './services/alarms';
import { LOG_PAGE, type LogbookApi, type LogEntry } from './services/logbook';

/** Nombre d'événements d'alarme chargés avec le journal. */
export const LOG_ALARM_EVENTS = 200;

/** Ligne du journal : entrée (manuelle, automatique) ou événement d'alarme. */
export type LogRow =
  | { type: 'entry'; id: string; at: number; entry: LogEntry }
  | { type: 'alarm'; id: string; at: number; event: AlarmEvent };

/**
 * Lignes du journal, de la plus récente à la plus ancienne. Tant que des
 * entrées plus anciennes restent à charger (`more`), les alarmes
 * antérieures à la plus ancienne entrée chargée attendent leur page.
 */
export function logRows(entries: LogEntry[], alarms: AlarmEvent[], more: boolean): LogRow[] {
  const oldest = entries.at(-1)?.at ?? -Infinity;
  const rows: LogRow[] = [
    ...entries.map((entry) => ({ type: 'entry' as const, id: entry.id, at: entry.at, entry })),
    ...alarms
      .filter((event) => !more || event.at >= oldest)
      .map((event) => ({ type: 'alarm' as const, id: `alarm-${event.id}`, at: event.at, event })),
  ];
  return rows.sort((a, b) => b.at - a.at);
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export class LogbookController {
  entries = $state.raw<LogEntry[]>([]);
  autoIntervalMin = $state<number | null>(null);
  /** D'autres entrées plus anciennes peuvent être chargées. */
  more = $state(false);
  error = $state<string | null>(null);
  notice = $state<string | null>(null);
  busy = $state(false);
  /** Événements d'alarme récents (moteur). */
  alarms = $state.raw<AlarmEvent[]>([]);
  /** Historique des alarmes illisible : signalé, le journal reste utilisable. */
  alarmsError = $state<string | null>(null);
  /** Entrées et alarmes intercalées, de la plus récente à la plus ancienne. */
  readonly rows: LogRow[] = $derived(logRows(this.entries, this.alarms, this.more));

  constructor(
    private readonly api: LogbookApi,
    /** Historique des alarmes ; absent : journal seul. */
    private readonly alarmEvents?: (limit: number) => Promise<AlarmEvent[]>,
  ) {}

  private async refreshAlarms(): Promise<void> {
    if (!this.alarmEvents) return;
    try {
      this.alarms = await this.alarmEvents(LOG_ALARM_EVENTS);
      this.alarmsError = null;
    } catch (e) {
      this.alarmsError = `Alarmes illisibles : ${message(e)}`;
    }
  }

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
    await this.refreshAlarms();
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
