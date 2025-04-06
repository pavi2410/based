import CodeMirror from '@uiw/react-codemirror';
import { sql } from '@codemirror/lang-sql';
import { json } from '@codemirror/lang-json';
import { useTheme } from '@/components/theme-provider';

export interface CodeEditorProps {
  value: string;
  onChange: (value: string) => void;
  language: 'sql' | 'json';
  className?: string;
  placeholder?: string;
}

export function CodeEditor({
  value,
  onChange,
  language,
  className,
  placeholder,
  ...props
}: CodeEditorProps) {
  const { theme } = useTheme();
  const isDark = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);

  const getLanguageExtension = () => {
    switch (language) {
      case 'sql':
        return sql();
      case 'json':
        return json();
      default:
        return sql();
    }
  };

  return (
    <CodeMirror
      value={value}
      onChange={onChange}
      extensions={[getLanguageExtension()]}
      theme={isDark ? 'dark' : 'light'}
      className={className}
      placeholder={placeholder}
      basicSetup={{
        lineNumbers: true,
        highlightActiveLine: true,
        highlightSelectionMatches: true,
        autocompletion: true,
      }}
      {...props}
    />
  );
} 