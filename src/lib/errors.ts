/**
 * Turns whatever a rejected IPC call carries into text for the user.
 *
 * The backend sends `{ code, message }` (`AppError`); Tauri itself may send a plain string; and
 * anything else is an unexpected failure that gets a generic message instead of `[object Object]`.
 */
export function describeError(error: unknown): string {
  if (typeof error === "string" && error.trim() !== "") return error;
  if (typeof error === "object" && error !== null) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim() !== "") return message;
  }
  return "Something went wrong.";
}
