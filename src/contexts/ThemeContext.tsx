import { createContext, useContext } from "react";

type ThemeContextValue = {
  darkMode: boolean;
};

export const ThemeContext = createContext<ThemeContextValue>({ darkMode: true });

export function useTheme(): ThemeContextValue {
  return useContext(ThemeContext);
}
