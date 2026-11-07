import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";

const GAP = 16;
const ROW_GAP = 18;
/** Title, meta line and spacing below the 4:3 cover. */
const TEXT_HEIGHT = 64;

type Props<T> = {
  items: T[];
  cardWidth: number;
  itemKey: (item: T) => string;
  renderItem: (item: T) => ReactNode;
  onNearEnd: () => void;
};

/** Only the visible rows are in the DOM, so 100k models scroll like 100. */
export function VirtualGrid<T>({ items, cardWidth, itemKey, renderItem, onNearEnd }: Props<T>) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const element = scrollRef.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => setWidth(entry?.contentRect.width ?? 0));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const columns = Math.max(1, Math.floor((width + GAP) / (cardWidth + GAP)));
  const actualWidth = width > 0 ? (width - GAP * (columns - 1)) / columns : cardWidth;
  const rowHeight = Math.round((actualWidth * 3) / 4 + TEXT_HEIGHT);
  const rowCount = Math.ceil(items.length / columns);
  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight + ROW_GAP,
    overscan: 3,
  });
  useEffect(() => virtualizer.measure(), [rowHeight, virtualizer]);

  const rows = virtualizer.getVirtualItems();
  const lastVisible = rows.at(-1)?.index ?? 0;
  useEffect(() => {
    if (rowCount > 0 && lastVisible >= rowCount - 3) onNearEnd();
  }, [lastVisible, rowCount, onNearEnd]);

  return (
    <div className="gridwrap" ref={scrollRef}>
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {rows.map((row) => (
          <div
            key={row.key}
            className="grow"
            style={{
              position: "absolute",
              top: 0,
              left: 0,
              right: 0,
              transform: `translateY(${row.start}px)`,
              gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
            }}
          >
            {items.slice(row.index * columns, (row.index + 1) * columns).map((item) => (
              <div key={itemKey(item)}>{renderItem(item)}</div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
