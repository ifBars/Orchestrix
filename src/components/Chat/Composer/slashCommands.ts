export type SlashCommandId = "compact" | "fork" | "new";

export interface SlashCommand {
  id: SlashCommandId;
  command: string;
  aliases: string[];
  description: string;
  icon?: string;
}

export const SLASH_COMMANDS: SlashCommand[] = [
  {
    id: "compact",
    command: "/compact",
    aliases: ["summarize", "summary", "summ"],
    description: "Compact conversation history by generating a summary",
  },
  {
    id: "fork",
    command: "/fork",
    aliases: ["branch"],
    description: "Fork the current task into a new conversation",
  },
  {
    id: "new",
    command: "/new",
    aliases: ["newchat", "fresh"],
    description: "Start a new conversation",
  },
];

export interface SlashMatch {
  command: SlashCommand;
  aliasMatched: string | null;
}

export function getSlashContext(
  text: string,
  cursor: number
): { start: number; query: string } | null {
  if (cursor < 0 || cursor > text.length) return null;

  let start = cursor - 1;
  while (start >= 0 && !/\s/.test(text[start])) {
    start -= 1;
  }
  start += 1;

  const token = text.slice(start, cursor);

  // Token must start with / and not contain whitespace or newline
  if (!token.startsWith("/")) return null;
  if (token.includes("\n")) return null;

  return { start, query: token };
}

export function matchSlashCommands(query: string): SlashMatch[] {
  const normalizedQuery = query.toLowerCase().trim();

  if (!normalizedQuery.startsWith("/")) {
    return [];
  }

  const queryWithoutSlash = normalizedQuery.slice(1);

  if (queryWithoutSlash.length === 0) {
    // Show all commands when user types just "/"
    return SLASH_COMMANDS.map((cmd) => ({
      command: cmd,
      aliasMatched: null,
    }));
  }

  const matches: SlashMatch[] = [];

  for (const cmd of SLASH_COMMANDS) {
    const cmdLower = cmd.command.toLowerCase();

    // Check if query matches the canonical command
    if (cmdLower.startsWith(normalizedQuery)) {
      // Canonical command prefix match - highest priority
      matches.push({ command: cmd, aliasMatched: null });
      continue;
    }

    // Check aliases
    for (const alias of cmd.aliases) {
      const aliasLower = alias.toLowerCase();
      // Match alias prefix (e.g., "summ" matches "summarize")
      if (aliasLower.startsWith(queryWithoutSlash)) {
        matches.push({ command: cmd, aliasMatched: alias });
        break;
      }
      // Also match if query is close to alias (fuzzy match for common abbreviations)
      if (queryWithoutSlash.length >= 2 && aliasLower.includes(queryWithoutSlash)) {
        matches.push({ command: cmd, aliasMatched: alias });
        break;
      }
    }
  }

  // Sort: canonical matches first, then by alias match quality
  matches.sort((a, b) => {
    // Canonical command prefix matches come first
    const aCanonical = a.aliasMatched === null;
    const bCanonical = b.aliasMatched === null;
    if (aCanonical && !bCanonical) return -1;
    if (!aCanonical && bCanonical) return 1;

    // Both are canonical or both are aliases - sort alphabetically
    return a.command.command.localeCompare(b.command.command);
  });

  // Remove duplicates (in case multiple aliases match)
  const seen = new Set<string>();
  return matches.filter((match) => {
    if (seen.has(match.command.id)) return false;
    seen.add(match.command.id);
    return true;
  });
}

export function findSlashCommand(
  commandId: SlashCommandId
): SlashCommand | undefined {
  return SLASH_COMMANDS.find((cmd) => cmd.id === commandId);
}
