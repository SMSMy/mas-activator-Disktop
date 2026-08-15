import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ShieldAlert, Loader2 } from "lucide-react";

interface ConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  confirmLabel: string;
  danger?: boolean;
  loading?: boolean;
  onConfirm: () => void;
  isDark: boolean;
}

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel,
  danger = false,
  loading = false,
  onConfirm,
  isDark,
}: ConfirmDialogProps) {
  return (
    <Dialog open={open} onOpenChange={(v) => !loading && onOpenChange(v)}>
      <DialogContent
        className={`rounded-xl max-w-md ${
          isDark
            ? "bg-gradient-to-br from-slate-900 to-slate-800 border border-cyan-500/30"
            : "bg-white border border-slate-200"
        }`}
        onPointerDownOutside={(e) => {
          if (loading) e.preventDefault();
        }}
      >
        <DialogHeader>
          <DialogTitle
            className={`text-xl flex items-center gap-2 ${isDark ? "text-cyan-300" : "text-[#1E293B]"}`}
          >
            <ShieldAlert className="w-5 h-5" />
            {title}
          </DialogTitle>
          <DialogDescription
            className={`text-sm leading-relaxed ${isDark ? "text-cyan-200/70" : "text-slate-600"}`}
          >
            {description}
          </DialogDescription>
        </DialogHeader>

        <div className="flex gap-3 pt-2">
          <Button
            onClick={onConfirm}
            disabled={loading}
            className={`flex-1 text-white disabled:opacity-50 ${
              danger
                ? "bg-red-600 hover:bg-red-700"
                : isDark
                ? "bg-cyan-600 hover:bg-cyan-700"
                : "bg-[#4682B4] hover:bg-[#3b75a5]"
            }`}
          >
            {loading ? <Loader2 className="w-4 h-4 ml-2 animate-spin" /> : null}
            {confirmLabel}
          </Button>
          <Button
            onClick={() => onOpenChange(false)}
            disabled={loading}
            variant="outline"
            className={`flex-1 ${isDark ? "bg-slate-700 hover:bg-slate-600 text-slate-200 border border-slate-500/30" : "bg-slate-100 hover:bg-slate-200 text-slate-700 border border-slate-200"}`}
          >
            رجوع
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
