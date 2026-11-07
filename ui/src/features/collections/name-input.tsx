import { useState } from "react";

type Props = { placeholder: string; initial?: string; onSubmit: (name: string) => void; onCancel: () => void };

/** One-line name field: Enter saves, Escape or leaving an empty field cancels. */
export function NameInput({ placeholder, initial = "", onSubmit, onCancel }: Props) {
  const [name, setName] = useState(initial);
  const submit = () => (name.trim() ? onSubmit(name.trim()) : onCancel());
  return (
    <input
      className="name-input"
      autoFocus
      value={name}
      placeholder={placeholder}
      aria-label={placeholder}
      onChange={(event) => setName(event.target.value)}
      onBlur={submit}
      onKeyDown={(event) => {
        if (event.key === "Enter") submit();
        if (event.key === "Escape") onCancel();
      }}
    />
  );
}
