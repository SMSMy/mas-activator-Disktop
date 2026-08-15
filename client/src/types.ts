export type LicenseState =
  | "activated"
  | "notification"
  | "grace"
  | "not_licensed"
  | "not_genuine"
  | "unknown";

export type ProductKind = "windows" | "office";

export interface ProductStatus {
  kind: ProductKind;
  name: string;
  state: LicenseState;
  label: string;
  grace_days: number | null;
  selection_reason: string;
}

export interface StatusReport {
  windows: ProductStatus | null;
  office: ProductStatus | null;
  observed: ProductStatus[];
  checked_at: string | null;
  error: { kind: string; message: string } | null;
}

export type OutcomeKind =
  | "verified_change"
  | "no_change"
  | "unverified"
  | "cancelled"
  | "timed_out"
  | "no_connection"
  | "blocked_by_protection"
  | "pin_refresh_required"
  | "failed";

export interface OperationOutcome {
  kind: OutcomeKind;
  label: string;
  message: string;
  before: string | null;
  after: string | null;
  checked_at: string | null;
  output_tail: string | null;
  pin_from: string | null;
  pin_to: string | null;
}

export type OpState = "idle" | "running" | "cancelling";

// ===== تغيير إصدار Windows (7.6) =====

export interface EditionSnapshot {
  product_name: string;
  edition_id: string;
  display_version: string | null;
  current_build: string;
  ubr: string;
  windows_state: LicenseState | null;
  windows_label: string | null;
  pending_file_rename: boolean;
  reboot_pending: boolean;
}

export interface EditionPreflightReport {
  current: EditionSnapshot | null;
  supported_targets: string[];
  blocked_targets: string[];
  checked_at: string | null;
  error: { kind: string; message: string } | null;
}

export type EditionChangeStatus =
  | "idle"
  | "preflight_ready"
  | "unsupported_path"
  | "discovery_failed"
  | "settings_opened"
  | "pending_restart"
  | "edition_changed_and_activated"
  | "edition_changed_needs_activation"
  | "edition_unchanged"
  | "verification_failed"
  | "cancelled"
  | "timed_out";

export interface EditionChangeResult {
  status: EditionChangeStatus;
  before: EditionSnapshot | null;
  after: EditionSnapshot | null;
  restart_required: boolean;
  checked_at: string | null;
  safe_message: string;
}
