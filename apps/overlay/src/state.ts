export type AuthState =
  | "Unauthenticated"
  | { WaitingForDeviceCode: { user_code: string; verification_uri: string; expires_in: number } }
  | { Connected: { account_id: string; player_name: string | null } }
  | "Expired"
  | { Error: { message: string } };

export interface PlayerCard {
  player_id: string;
  name: string | null;
  playlist: number | null;
  mmr: number | null;
  tier: number | null;
  division: number | null;
  data_age_seconds: number;
}

export interface OverlayState {
  auth: AuthState;
  players: PlayerCard[];
  partial_roster: boolean;
  status_message: string;
}

export function authLabel(auth: AuthState): string {
  if (auth === "Unauthenticated") return "Auth required";
  if (auth === "Expired") return "Auth expired";
  if (typeof auth === "object" && "WaitingForDeviceCode" in auth) return "Waiting for login";
  if (typeof auth === "object" && "Connected" in auth) return "Connected";
  if (typeof auth === "object" && "Error" in auth) return `Error: ${auth.Error.message}`;
  return "Unknown";
}
