// ============ Log Viewer Component ============
// مكون مستقل لعرض سجلات التنفيذ

import { motion } from "framer-motion";

interface LogViewerProps {
  logs: string[];
  isDark: boolean;
  /** الحد الأقصى للارتفاع (Tailwind class) — افتراضي max-h-48 */
  maxHeightClass?: string;
  /** إضافة عنوان "السجل:" قبل الإدخالات */
  showLabel?: boolean;
}

export function LogViewer({
  logs,
  isDark,
  maxHeightClass = "max-h-48",
  showLabel = false,
}: LogViewerProps) {
  if (logs.length === 0) return null;

  return (
    <motion.div
      className={`rounded-lg p-3 ${maxHeightClass} overflow-y-auto ${
        isDark
          ? "bg-black/40 border border-cyan-500/20"
          : "bg-[#F8FAFC] border border-slate-200"
      }`}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      transition={{ delay: 0.2 }}
    >
      {showLabel && (
        <p className={`text-xs mb-2 font-semibold ${isDark ? "text-cyan-300/60" : "text-slate-500"}`}>
          السجل:
        </p>
      )}
      <div className="space-y-1">
        {logs.map((log, idx) => (
          <p
            key={idx}
            className={`text-xs font-mono ${
              isDark ? "text-cyan-400/80" : "text-slate-600"
            }`}
          >
            {log}
          </p>
        ))}
      </div>
    </motion.div>
  );
}
