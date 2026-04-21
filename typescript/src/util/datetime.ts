/**
 * ISO 8601/RFC 3339 date/time parsing and formatting utilities.
 *
 * Converts between GRC-20 temporal strings and internal representations:
 * - Date: days since Unix epoch (1970-01-01) + offset in minutes
 * - Time: microseconds since midnight + offset in minutes
 * - Datetime: microseconds since Unix epoch + offset in minutes
 *
 * DATE and DATETIME use ISO 8601 extended forms with astronomical year numbering
 * and expanded years. TIME keeps the RFC 3339-compatible clock syntax.
 */

const MICROSECONDS_PER_SECOND = 1_000_000n;
const MICROSECONDS_PER_MINUTE = 60n * MICROSECONDS_PER_SECOND;
const MICROSECONDS_PER_HOUR = 60n * MICROSECONDS_PER_MINUTE;
const MICROSECONDS_PER_DAY = 24n * 60n * 60n * 1_000_000n;

/**
 * Parses a timezone offset string (Z, +HH:MM, -HH:MM) and returns offset in minutes.
 */
function parseTimezoneOffset(offset: string): number {
  if (offset === "Z" || offset === "z") {
    return 0;
  }

  const match = offset.match(/^([+-])(\d{2}):(\d{2})$/);
  if (!match) {
    throw new Error(`Invalid timezone offset: ${offset}`);
  }

  const sign = match[1] === "+" ? 1 : -1;
  const hours = parseInt(match[2], 10);
  const minutes = parseInt(match[3], 10);

  if (hours > 23 || minutes > 59) {
    throw new Error(`Invalid timezone offset: ${offset}`);
  }

  const totalMinutes = sign * (hours * 60 + minutes);
  if (totalMinutes < -1440 || totalMinutes > 1440) {
    throw new Error(`Timezone offset out of range [-24:00, +24:00]: ${offset}`);
  }

  return totalMinutes;
}

/**
 * Formats an offset in minutes as a timezone string (Z, +HH:MM, -HH:MM).
 */
function formatTimezoneOffset(offsetMin: number): string {
  if (offsetMin === 0) {
    return "Z";
  }

  const sign = offsetMin >= 0 ? "+" : "-";
  const absOffset = Math.abs(offsetMin);
  const hours = Math.floor(absOffset / 60);
  const minutes = absOffset % 60;

  return `${sign}${hours.toString().padStart(2, "0")}:${minutes.toString().padStart(2, "0")}`;
}

/**
 * Parses fractional seconds string and returns microseconds.
 */
function parseFractionalSeconds(frac: string | undefined): bigint {
  if (!frac) {
    return 0n;
  }

  // Pad or truncate to 6 digits (microseconds)
  const padded = frac.padEnd(6, "0").slice(0, 6);
  return BigInt(parseInt(padded, 10));
}

/**
 * Formats microseconds as fractional seconds string, omitting if zero.
 */
function formatFractionalSeconds(us: bigint): string {
  if (us === 0n) {
    return "";
  }

  // Convert to 6-digit string and trim trailing zeros
  const str = us.toString().padStart(6, "0");
  const trimmed = str.replace(/0+$/, "");
  return `.${trimmed}`;
}

function isLeapYear(year: number): boolean {
  return (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
}

function daysInMonth(year: number, month: number): number {
  switch (month) {
    case 1:
    case 3:
    case 5:
    case 7:
    case 8:
    case 10:
    case 12:
      return 31;
    case 4:
    case 6:
    case 9:
    case 11:
      return 30;
    case 2:
      return isLeapYear(year) ? 29 : 28;
    default:
      return 0;
  }
}

function dateToDays(year: number, month: number, day: number): number {
  const y = month <= 2 ? year - 1 : year;
  const m = month <= 2 ? month + 9 : month - 3;
  const era = Math.trunc((y >= 0 ? y : y - 399) / 400);
  const yoe = y - era * 400;
  const doy = Math.floor((153 * m + 2) / 5) + day - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  return era * 146097 + doe - 719468;
}

function daysToDate(days: number): { year: number; month: number; day: number } {
  const z = days + 719468;
  const era = Math.trunc((z >= 0 ? z : z - 146096) / 146097);
  const doe = z - era * 146097;
  const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
  const y = yoe + era * 400;
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
  const mp = Math.floor((5 * doy + 2) / 153);
  const day = doy - Math.floor((153 * mp + 2) / 5) + 1;
  const month = mp < 10 ? mp + 3 : mp - 9;
  const year = month <= 2 ? y + 1 : y;
  return { year, month, day };
}

function parseCalendarYearPrefix(input: string, context: string): { year: number; firstDash: number } {
  const signed = input.startsWith("+") || input.startsWith("-");
  const digitStart = signed ? 1 : 0;
  const firstDash = input.indexOf("-", digitStart);
  if (firstDash === -1) {
    throw new Error(`Invalid ${context}: ${input}`);
  }

  const yearDigits = input.slice(digitStart, firstDash);
  if (yearDigits.length < 4 || (!signed && yearDigits.length !== 4) || !/^\d+$/.test(yearDigits)) {
    throw new Error(`Invalid year in ${context}: ${input}`);
  }

  const year = parseInt(input.slice(0, firstDash), 10);
  if (!Number.isInteger(year)) {
    throw new Error(`Invalid year in ${context}: ${input}`);
  }

  return { year, firstDash };
}

function parseCalendarDatePrefix(
  input: string,
  context: string
): { year: number; month: number; day: number; consumed: number } {
  const { year, firstDash } = parseCalendarYearPrefix(input, context);
  if (input.length < firstDash + 6 || input[firstDash + 3] !== "-") {
    throw new Error(`Invalid ${context}: ${input}`);
  }

  const month = parseInt(input.slice(firstDash + 1, firstDash + 3), 10);
  const day = parseInt(input.slice(firstDash + 4, firstDash + 6), 10);

  if (month < 1 || month > 12) {
    throw new Error(`Invalid month in ${context}: ${input}`);
  }
  if (day < 1 || day > daysInMonth(year, month)) {
    throw new Error(`Invalid day in ${context}: ${input}`);
  }

  return { year, month, day, consumed: firstDash + 6 };
}

function formatCalendarYear(year: number): string {
  if (year >= 0 && year <= 9999) {
    return year.toString().padStart(4, "0");
  }
  if (year < 0) {
    return `-${Math.abs(year).toString().padStart(4, "0")}`;
  }
  return `+${year.toString().padStart(5, "0")}`;
}

function floorDivBigInt(value: bigint, divisor: bigint): bigint {
  let quotient = value / divisor;
  const remainder = value % divisor;
  if (remainder < 0n) {
    quotient -= 1n;
  }
  return quotient;
}

// =====================
// DATE functions
// =====================

/**
 * Parses a GRC-20 DATE string and returns days since Unix epoch.
 *
 * DATE strings use ISO 8601 extended calendar dates with astronomical year
 * numbering and expanded years. Examples:
 * - `2024-03-15`
 * - `0000-01-01Z`
 * - `-0001-12-31+05:30`
 * - `+12024-03-15Z`
 */
export function parseDateRfc3339(dateStr: string): { days: number; offsetMin: number } {
  const { year, month, day, consumed } = parseCalendarDatePrefix(dateStr, "ISO 8601 date");
  const days = dateToDays(year, month, day);
  const offsetStr = dateStr.slice(consumed);
  const offsetMin = offsetStr ? parseTimezoneOffset(offsetStr) : 0;

  return { days, offsetMin };
}

/**
 * Formats days since Unix epoch as a canonical GRC-20 DATE string.
 */
export function formatDateRfc3339(days: number, offsetMin: number = 0): string {
  const { year, month, day } = daysToDate(days);

  const offset = formatTimezoneOffset(offsetMin);
  return `${formatCalendarYear(year)}-${month.toString().padStart(2, "0")}-${day.toString().padStart(2, "0")}${offset}`;
}

// =====================
// TIME functions
// =====================

/**
 * Parses an RFC 3339 time string (HH:MM:SS[.ssssss][Z|+HH:MM]) and returns
 * microseconds since midnight and offset in minutes.
 *
 * Spec: TIME value definition (spec.md "TIME" section) requires offset_min;
 * reject inputs without explicit timezone (Z or ±HH:MM).
 */
export function parseTimeRfc3339(timeStr: string): { timeMicros: bigint; offsetMin: number } {
  // Match HH:MM:SS[.fractional][timezone]
  const match = timeStr.match(
    /^(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,6}))?(Z|[+-]\d{2}:\d{2})$/
  );
  if (!match) {
    throw new Error(`Invalid RFC 3339 time: ${timeStr}`);
  }

  const hours = parseInt(match[1], 10);
  const minutes = parseInt(match[2], 10);
  const seconds = parseInt(match[3], 10);
  const fractional = match[4];
  const offsetStr = match[5];
  if (!offsetStr) {
    throw new Error(`Timezone offset required in time: ${timeStr}`);
  }

  // Validate ranges
  if (hours > 23) {
    throw new Error(`Invalid hours in time: ${timeStr}`);
  }
  if (minutes > 59) {
    throw new Error(`Invalid minutes in time: ${timeStr}`);
  }
  if (seconds > 59) {
    throw new Error(`Invalid seconds in time: ${timeStr}`);
  }

  const microseconds = parseFractionalSeconds(fractional);
  const timeMicros =
    BigInt(hours) * MICROSECONDS_PER_HOUR +
    BigInt(minutes) * MICROSECONDS_PER_MINUTE +
    BigInt(seconds) * MICROSECONDS_PER_SECOND +
    microseconds;

  // Validate total is within day
  if (timeMicros > 86_399_999_999n) {
    throw new Error(`Time exceeds maximum (23:59:59.999999): ${timeStr}`);
  }

  const offsetMin = parseTimezoneOffset(offsetStr);

  return { timeMicros, offsetMin };
}

/**
 * Formats microseconds since midnight as RFC 3339 time string.
 */
export function formatTimeRfc3339(timeMicros: bigint, offsetMin: number = 0): string {
  const hours = Number(timeMicros / MICROSECONDS_PER_HOUR);
  const remaining1 = timeMicros % MICROSECONDS_PER_HOUR;
  const minutes = Number(remaining1 / MICROSECONDS_PER_MINUTE);
  const remaining2 = remaining1 % MICROSECONDS_PER_MINUTE;
  const seconds = Number(remaining2 / MICROSECONDS_PER_SECOND);
  const microseconds = remaining2 % MICROSECONDS_PER_SECOND;

  const hh = hours.toString().padStart(2, "0");
  const mm = minutes.toString().padStart(2, "0");
  const ss = seconds.toString().padStart(2, "0");
  const frac = formatFractionalSeconds(microseconds);
  const offset = formatTimezoneOffset(offsetMin);

  return `${hh}:${mm}:${ss}${frac}${offset}`;
}

// =====================
// DATETIME functions
// =====================

/**
 * Parses a GRC-20 DATETIME string and returns microseconds since Unix epoch
 * and offset in minutes.
 *
 * DATETIME strings use ISO 8601 extended calendar dates with astronomical year
 * numbering and expanded years, plus RFC 3339-compatible time and offset syntax.
 * The offset is required.
 */
export function parseDatetimeRfc3339(datetimeStr: string): { epochMicros: bigint; offsetMin: number } {
  const { year, month, day, consumed } = parseCalendarDatePrefix(datetimeStr, "ISO 8601 datetime");
  const separator = datetimeStr[consumed];
  if (separator !== "T" && separator !== " ") {
    throw new Error(`Invalid ISO 8601 datetime: ${datetimeStr}`);
  }

  const timeStr = datetimeStr.slice(consumed + 1);
  const match = timeStr.match(
    /^(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,6}))?(Z|[+-]\d{2}:\d{2})$/
  );
  if (!match) {
    throw new Error(`Invalid ISO 8601 datetime: ${datetimeStr}`);
  }

  const hours = parseInt(match[1], 10);
  const minutes = parseInt(match[2], 10);
  const seconds = parseInt(match[3], 10);
  const fractional = match[4];
  const offsetStr = match[5];
  if (!offsetStr) {
    throw new Error(`Timezone offset required in datetime: ${datetimeStr}`);
  }

  // Validate ranges
  if (hours > 23) {
    throw new Error(`Invalid hours in datetime: ${datetimeStr}`);
  }
  if (minutes > 59) {
    throw new Error(`Invalid minutes in datetime: ${datetimeStr}`);
  }
  if (seconds > 59) {
    throw new Error(`Invalid seconds in datetime: ${datetimeStr}`);
  }

  const offsetMin = parseTimezoneOffset(offsetStr);
  const microseconds = parseFractionalSeconds(fractional);

  const epochMicrosUTC =
    BigInt(dateToDays(year, month, day)) * MICROSECONDS_PER_DAY +
    BigInt(hours) * MICROSECONDS_PER_HOUR +
    BigInt(minutes) * MICROSECONDS_PER_MINUTE +
    BigInt(seconds) * MICROSECONDS_PER_SECOND +
    microseconds;

  const offsetUs = BigInt(offsetMin) * MICROSECONDS_PER_MINUTE;
  const epochMicros = epochMicrosUTC - offsetUs;

  return { epochMicros, offsetMin };
}

/**
 * Formats microseconds since Unix epoch as a canonical GRC-20 DATETIME string.
 */
export function formatDatetimeRfc3339(epochMicros: bigint, offsetMin: number = 0): string {
  const offsetUs = BigInt(offsetMin) * MICROSECONDS_PER_MINUTE;
  const localUs = epochMicros + offsetUs;

  const days = floorDivBigInt(localUs, MICROSECONDS_PER_DAY);
  const timeMicros = localUs - days * MICROSECONDS_PER_DAY;
  const { year, month, day } = daysToDate(Number(days));

  const hours = timeMicros / MICROSECONDS_PER_HOUR;
  const remaining1 = timeMicros % MICROSECONDS_PER_HOUR;
  const minutes = remaining1 / MICROSECONDS_PER_MINUTE;
  const remaining2 = remaining1 % MICROSECONDS_PER_MINUTE;
  const seconds = remaining2 / MICROSECONDS_PER_SECOND;
  const microseconds = remaining2 % MICROSECONDS_PER_SECOND;

  const frac = formatFractionalSeconds(microseconds);
  const offset = formatTimezoneOffset(offsetMin);

  return `${formatCalendarYear(year)}-${month.toString().padStart(2, "0")}-${day
    .toString()
    .padStart(2, "0")}T${hours.toString().padStart(2, "0")}:${minutes
    .toString()
    .padStart(2, "0")}:${seconds.toString().padStart(2, "0")}${frac}${offset}`;
}
