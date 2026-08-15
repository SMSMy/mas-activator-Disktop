import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Loader2, Zap, Monitor, FileText, CheckCircle2, Sun, Moon, PartyPopper, Ban, ArrowLeftRight, ShieldAlert, FileDown } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import type { TargetAndTransition } from "framer-motion";
import { useState, useEffect, useRef, useCallback } from "react";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { useTheme } from "@/contexts/ThemeContext";
import { Confetti } from "@/components/ui/Confetti";
import { LogViewer } from "@/components/ui/LogViewer";
import { StatusDialog } from "@/components/ui/StatusDialog";
import { EditionDialog } from "@/components/EditionDialog";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import type { OperationOutcome, OpState, StatusReport } from "@/types";

type ActivationAction = {
  label: string;
  sub: string;
  kind: string;
  name: string;
};

export default function Home() {
  const { theme, toggleTheme } = useTheme();
  const [opState, setOpState] = useState<OpState>("idle");
  const [activeAction, setActiveAction] = useState<string | null>(null);
  const [statusDialog, setStatusDialog] = useState(false);
  const [status, setStatus] = useState({ windows: "جار الفحص...", office: "جار الفحص..." });
  const [statusLoading, setStatusLoading] = useState(false);
  const [statusResult, setStatusResult] = useState<"none" | "success" | "error">("none");
  const [logs, setLogs] = useState<string[]>([]);
  const [showLogs, setShowLogs] = useState(false);
  const [celebratingKey, setCelebratingKey] = useState<string | null>(null);
  const [confettiOrigin, setConfettiOrigin] = useState<{ x: number; y: number } | null>(null);
  const [editionDialog, setEditionDialog] = useState(false);
  const [version, setVersion] = useState("");
  const [isAdmin, setIsAdmin] = useState<boolean | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [pendingAction, setPendingAction] = useState<{ action: ActivationAction; buttonKey: string } | null>(null);
  const [protectionBlocked, setProtectionBlocked] = useState(false);
  const [pinAdoption, setPinAdoption] = useState<{ from: string; to: string } | null>(null);
  const [adopting, setAdopting] = useState(false);
  const buttonRefs = useRef<Record<string, HTMLDivElement | null>>({});

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(""));
    invoke<boolean>("check_admin")
      .then(setIsAdmin)
      .catch(() => setIsAdmin(null));
  }, []);

  const addLog = (message: string) => {
    setLogs((prev) => [...prev, `[${new Date().toLocaleTimeString()}] ${message}`]);
  };

  useEffect(() => {
    if (statusResult !== "none") {
      const timer = setTimeout(() => setStatusResult("none"), 5000);
      return () => clearTimeout(timer);
    }
  }, [statusResult]);

  const triggerCelebration = useCallback((key: string) => {
    const el = buttonRefs.current[key];
    if (el) {
      const rect = el.getBoundingClientRect();
      setConfettiOrigin({ x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 });
    }
    setCelebratingKey(key);
    setTimeout(() => setCelebratingKey(null), 2500);
  }, []);

  const busy = opState === "running" || opState === "cancelling";

  const handleActivation = async (action: ActivationAction, buttonKey: string) => {
    if (busy) return;
    setOpState("running");
    setActiveAction(action.name);
    setLogs([]);
    setProtectionBlocked(false);
    addLog(`جاري تنفيذ: ${action.label}`);

    try {
      const outcome = await invoke<OperationOutcome>("run_activation", { kind: action.kind });
      addLog(`[${outcome.label}] ${outcome.message}`);
      if (outcome.checked_at) {
        addLog(`🕐 وقت الفحص: ${outcome.checked_at}`);
      }
      if (outcome.output_tail) {
        addLog(outcome.output_tail.slice(-500));
      }

      switch (outcome.kind) {
        case "verified_change":
          toast.success(outcome.message);
          triggerCelebration(buttonKey);
          await handleCheckStatus();
          break;
        case "no_change":
          toast.info(outcome.message);
          break;
        case "unverified":
          toast.warning(outcome.message);
          break;
        case "cancelled":
          toast.info("تم إلغاء العملية");
          break;
        case "timed_out":
          toast.error("انتهت مهلة العملية وأُنهيت");
          break;
        case "no_connection":
          toast.error("لا يوجد اتصال بالإنترنت");
          break;
        case "blocked_by_protection":
          toast.error(outcome.message);
          setProtectionBlocked(true);
          break;
        case "pin_refresh_required":
          toast.warning(outcome.message);
          setPinAdoption({
            from: outcome.pin_from || "؟",
            to: outcome.pin_to || "؟",
          });
          break;
        default:
          toast.error(outcome.message);
          break;
      }
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      addLog(`❌ ${errorMsg}`);
      toast.error(`حدث خطأ: ${errorMsg}`);
    } finally {
      setOpState("idle");
      setActiveAction(null);
    }
  };

  const handleCancel = async () => {
    if (opState !== "running") return;
    setOpState("cancelling");
    addLog("⏹️ جاري إلغاء العملية...");
    try {
      await invoke("cancel_operation");
    } catch {
      // العملية ربما انتهت بالفعل
    }
  };

  const requestActivation = (action: ActivationAction, buttonKey: string) => {
    if (busy) return;
    setPendingAction({ action, buttonKey });
    setConfirmOpen(true);
  };

  const confirmAndRun = async () => {
    if (!pendingAction) return;
    const { action, buttonKey } = pendingAction;
    setConfirmOpen(false);
    setPendingAction(null);
    await handleActivation(action, buttonKey);
  };

  const handleExportReport = async () => {
    try {
      const path = await invoke<string>("export_logs");
      addLog(`📄 حُفظ التقرير التشخيصي في: ${path}`);
      toast.success("حُفظ التقرير التشخيصي في مجلد التنزيلات");
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      addLog(`❌ ${msg}`);
      toast.error(`تعذر حفظ التقرير: ${msg}`);
    }
  };

  const handleOpenWindowsSecurity = async () => {
    try {
      const message = await invoke<string>("open_windows_security");
      addLog(`🛡️ ${message}`);
      toast.info(message);
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      toast.error(msg);
    }
  };

  const handleAdoptPin = async () => {
    if (!pinAdoption) return;
    setAdopting(true);
    try {
      const message = await invoke<string>("adopt_mas_pin");
      addLog(`🔒 ${message}`);
      toast.success(message);
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      addLog(`❌ ${msg}`);
      toast.error(msg);
    } finally {
      setAdopting(false);
      setPinAdoption(null);
    }
  };

  const handleCheckStatus = async () => {
    if (busy) return;
    setStatusDialog(true);
    setStatusLoading(true);
    setStatusResult("none");
    setStatus({ windows: "جار الفحص...", office: "جار الفحص..." });
    setLogs([]);
    addLog("🔍 جاري فحص حالة التفعيل...");

    try {
      const report = await invoke<StatusReport>("check_status");

      const winStatus = report.windows ? report.windows.label : "غير مثبت";
      const officeStatus = report.office
        ? `${report.office.label}${report.office.name ? ` (${report.office.name})` : ""}`
        : "غير مثبت";

      setStatus({ windows: winStatus, office: officeStatus });

      addLog(`✅ اكتمل الفحص${report.checked_at ? ` — ${report.checked_at}` : ""}`);
      addLog(`💻 ويندوز: ${report.windows ? `${report.windows.name} — ${report.windows.label} (${report.windows.selection_reason})` : "غير مثبت"}`);
      addLog(`📄 أوفيس: ${report.office ? `${report.office.name} — ${report.office.label} (${report.office.selection_reason})` : "غير مثبت"}`);
      for (const p of report.observed.slice(0, 5)) {
        addLog(`   • ${p.name} — ${p.label}`);
      }

      if (report.error) {
        addLog(`⚠️ ملاحظة: ${report.error.message}`);
      }

      setStatusResult("success");
      triggerCelebration("status");
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      addLog(`❌ فشل الفحص: ${errorMsg}`);
      toast.error("تعذّر فحص الحالة");
      setStatus({ windows: "خطأ في الفحص", office: "خطأ في الفحص" });
      setStatusResult("error");
    } finally {
      setStatusLoading(false);
    }
  };

  const isDark = theme === "dark";

  const containerVariants = {
    hidden: { opacity: 0 },
    visible: {
      opacity: 1,
      transition: { staggerChildren: 0.1, delayChildren: 0.2 },
    },
  };

  const itemVariants = {
    hidden: { opacity: 0, y: 20 },
    visible: { opacity: 1, y: 0 },
  };

  const cardHover = {
    scale: 1.03,
    y: -4,
    transition: { type: "spring" as const, stiffness: 400, damping: 20 },
  } satisfies TargetAndTransition;

  const cardTap = { scale: 0.97 };

  const cardStyles = {
    windows: isDark
      ? "bg-gradient-to-br from-blue-600 to-blue-800 border-blue-400/30 hover:from-blue-500 hover:to-blue-700"
      : "bg-[#4682B4] hover:bg-[#3b6c9e]",
    office: isDark
      ? "bg-gradient-to-br from-orange-600 to-red-800 border-orange-400/30 hover:from-orange-500 hover:to-red-700"
      : "bg-[#94A3B8] hover:bg-[#7e8ea3]",
    all: isDark
      ? "bg-gradient-to-br from-green-600 to-emerald-800 border-green-400/30 hover:from-green-500 hover:to-emerald-700"
      : "bg-[#475569] hover:bg-[#374151]",
    status: isDark
      ? "bg-gradient-to-br from-purple-600 to-pink-800 border-purple-400/30 hover:from-purple-500 hover:to-pink-700"
      : "bg-[#0F172A] hover:bg-[#1e293b]",
  };

  const mainActions: Array<{ key: "windows" | "office" | "all"; action: ActivationAction; icon: typeof Monitor }> = [
    { key: "windows", action: { label: "تفعيل ويندوز", sub: "HWID Activation", kind: "windows", name: "تفعيل ويندوز" }, icon: Monitor },
    { key: "office", action: { label: "تفعيل أوفيس", sub: "Ohook Activation", kind: "office", name: "تفعيل أوفيس" }, icon: FileText },
    { key: "all", action: { label: "تفعيل الكل", sub: "Windows + Office", kind: "all", name: "تفعيل الكل" }, icon: Zap },
  ];

  return (
    <div className="min-h-screen grid-bg-animated relative overflow-hidden" dir="rtl">
      {/* Top bar: theme toggle (left) + credits (right) */}
      <div className="absolute top-4 left-4 right-4 z-50 flex items-center justify-between">
        <button
          onClick={toggleTheme}
          className="p-2.5 rounded-full bg-card border border-border hover:border-ring transition-all duration-300 cursor-pointer"
          title={isDark ? "الوضع النهاري" : "الوضع الليلي"}
        >
          {isDark ? (
            <Sun className="w-5 h-5 text-yellow-400" />
          ) : (
            <Moon className="w-5 h-5 text-slate-600" />
          )}
        </button>
        <span className={`text-xs tracking-wide ${isDark ? "text-cyan-300/40" : "text-slate-400"}`}>
          تطوير: يزيد يحيى{version ? ` | الإصدار ${version}` : ""}
        </span>
      </div>

      {/* Main content */}
      <motion.div
        className="relative z-10 min-h-screen flex flex-col items-center justify-center px-4 py-12"
        variants={containerVariants}
        initial="hidden"
        animate="visible"
        transition={{ staggerChildren: 0.1, delayChildren: 0.2 }}
      >
        {/* Header */}
        <motion.div className="text-center mb-16" variants={itemVariants}>
          {isAdmin === false && (
            <div
              className={`mb-6 px-4 py-3 rounded-lg text-sm flex items-center justify-center gap-2 ${
                isDark
                  ? "bg-amber-900/40 border border-amber-500/40 text-amber-200"
                  : "bg-amber-50 border border-amber-300 text-amber-800"
              }`}
            >
              <ShieldAlert className="w-4 h-4 shrink-0" />
              التطبيق يعمل بدون صلاحيات المسؤول — عمليات التفعيل والتغيير لن تنجح. أغلق التطبيق وأعد تشغيله «كمسؤول».
            </div>
          )}
          <div className="mb-4 inline-flex items-center gap-3">
            <Monitor className={`w-10 h-10 ${isDark ? "text-cyan-400" : "text-[#4682B4]"}`} />
          </div>
          <motion.h1
            className={`text-5xl md:text-7xl font-black mb-4 ${
              isDark
                ? "bg-gradient-to-r from-cyan-400 via-blue-400 to-purple-400 bg-clip-text text-transparent"
                : "text-[#1E293B]"
            }`}
          >
            M A S
          </motion.h1>
          <motion.p className={`text-lg font-light tracking-widest ${isDark ? "text-cyan-300/80" : "text-[#64748B]"}`}>
            أداة التفعيل الشاملة
          </motion.p>
          <motion.p className={`text-xs mt-1 tracking-wide ${isDark ? "text-cyan-300/40" : "text-[#94A3B8]"}`}>
            Microsoft Activation Scripts
          </motion.p>
        </motion.div>

        {/* Main buttons grid */}
        <motion.div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-12 max-w-2xl w-full" variants={itemVariants}>
          {mainActions.map(({ key, action, icon: Icon }) => {
            const isThisLoading = activeAction === action.name && busy;
            const isCelebrating = celebratingKey === key;

            return (
              <motion.div
                key={key}
                ref={(el) => { buttonRefs.current[key] = el; }}
                whileHover={cardHover}
                whileTap={cardTap}
                animate={isCelebrating ? {
                  scale: [1, 1.08, 1.03, 1.06, 1],
                  transition: { duration: 0.6, ease: "easeOut" }
                } : {}}
              >
                <Button
                  onClick={() => requestActivation(action, key)}
                  disabled={busy}
                  className={`w-full h-32 rounded-xl text-lg font-bold flex flex-col items-center justify-center gap-3 transition-all duration-300 shadow-lg ${cardStyles[key]} text-white relative overflow-hidden ${
                    isCelebrating ? 'ring-4 ring-green-400/70 shadow-[0_0_30px_rgba(74,222,128,0.4)]' : ''
                  }`}
                >
                  {isCelebrating && (
                    <motion.div
                      className="absolute inset-0 bg-gradient-to-r from-transparent via-white/25 to-transparent"
                      initial={{ x: '-100%' }}
                      animate={{ x: '200%' }}
                      transition={{ duration: 0.8, ease: 'easeInOut' }}
                    />
                  )}
                  <AnimatePresence mode="wait">
                    {isThisLoading ? (
                      <motion.div
                        key="loader"
                        initial={{ opacity: 0, scale: 0.5 }}
                        animate={{ opacity: 1, scale: 1 }}
                        exit={{ opacity: 0, scale: 0.5 }}
                      >
                        <Loader2 className="w-8 h-8 animate-spin text-white" />
                      </motion.div>
                    ) : isCelebrating ? (
                      <motion.div
                        key="celebrate"
                        initial={{ opacity: 0, scale: 0, rotate: -180 }}
                        animate={{ opacity: 1, scale: 1, rotate: 0 }}
                        exit={{ opacity: 0, scale: 0.5 }}
                        transition={{ type: 'spring', stiffness: 300, damping: 15 }}
                      >
                        <PartyPopper className="w-8 h-8 text-yellow-300 drop-shadow-[0_0_8px_rgba(253,224,71,0.6)]" />
                      </motion.div>
                    ) : (
                      <motion.div
                        key="icon"
                        initial={{ opacity: 0, scale: 0.5 }}
                        animate={{ opacity: 1, scale: 1 }}
                        exit={{ opacity: 0, scale: 0.5 }}
                      >
                        <Icon className="w-8 h-8 text-white" />
                      </motion.div>
                    )}
                  </AnimatePresence>
                  <span className="text-white relative z-10">{isCelebrating ? '✅ تم بنجاح!' : action.label}</span>
                  <span className="text-xs text-white/60 relative z-10">{action.sub}</span>
                </Button>
              </motion.div>
            );
          })}

          {/* Status card */}
          <motion.div
            ref={(el) => { buttonRefs.current["status"] = el; }}
            whileHover={cardHover}
            whileTap={cardTap}
          >
            <Button
              onClick={handleCheckStatus}
              disabled={busy}
              className={`w-full h-32 rounded-xl text-lg font-bold flex flex-col items-center justify-center gap-3 transition-all duration-300 shadow-lg ${cardStyles.status} text-white relative overflow-hidden`}
            >
              {statusLoading ? (
                <Loader2 className="w-8 h-8 animate-spin text-white" />
              ) : (
                <CheckCircle2 className="w-8 h-8 text-white" />
              )}
              <span className="text-white relative z-10">فحص الحالة</span>
              <span className="text-xs text-white/60 relative z-10">Status Check</span>
            </Button>
          </motion.div>
        </motion.div>

        {/* Cancel button while running */}
        {busy && (
          <motion.div className="mb-8" initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }}>
            <Button
              onClick={handleCancel}
              disabled={opState === "cancelling"}
              className="rounded-lg px-6 py-3 font-semibold bg-red-600/80 hover:bg-red-700 text-white border border-red-400/30 disabled:opacity-50"
            >
              {opState === "cancelling" ? (
                <Loader2 className="w-4 h-4 animate-spin ml-2" />
              ) : (
                <Ban className="w-4 h-4 ml-2" />
              )}
              {opState === "cancelling" ? "جاري الإلغاء..." : "إلغاء العملية"}
            </Button>
          </motion.div>
        )}

        {/* Protection blocked banner (يظهر فقط عند الحجب) */}
        {protectionBlocked && !busy && (
          <motion.div
            className={`mb-8 w-full max-w-2xl rounded-lg p-4 space-y-2 ${
              isDark
                ? "bg-amber-900/30 border border-amber-500/40"
                : "bg-amber-50 border border-amber-300"
            }`}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
          >
            <p className={`text-sm font-semibold flex items-center gap-2 ${isDark ? "text-amber-200" : "text-amber-800"}`}>
              <ShieldAlert className="w-4 h-4 shrink-0" />
              تم حظر العملية بواسطة حماية النظام
            </p>
            <p className={`text-xs leading-relaxed ${isDark ? "text-amber-100/80" : "text-amber-800"}`}>
              تذكير: أي تغيير في إعدادات الحماية قرارك — أعد التمكين فور انتهاء العملية. إن وثقت بمصدر هذا التطبيق يمكنك استثناء ملفه من الفحص في حماية Windows.
            </p>
            <Button
              onClick={handleOpenWindowsSecurity}
              className={`${isDark ? "bg-amber-700/70 hover:bg-amber-700 text-white border border-amber-400/30" : "bg-amber-600 hover:bg-amber-700 text-white"}`}
            >
              فتح حماية Windows
            </Button>
          </motion.div>
        )}

        {/* Advanced Options */}
        <motion.div className="w-full max-w-2xl" variants={itemVariants}>
          <div className="text-center mb-6">
            <p className={`text-sm tracking-widest ${isDark ? "text-cyan-300/60" : "text-slate-400"}`}>
              خيارات متقدمة
            </p>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Tooltip>
              <TooltipTrigger asChild>
                <motion.div whileHover={{ scale: 1.02 }} whileTap={{ scale: 0.97 }}>
                  <Button
                    onClick={() => requestActivation({ label: "TSforge (الكل)", sub: "", kind: "tsforge", name: "TSforge" }, "tsforge")}
                    disabled={busy}
                    className={`w-full rounded-lg py-6 font-semibold transition-all duration-300 ${
                      isDark
                        ? "bg-gradient-to-br from-violet-600 to-indigo-800 hover:from-violet-500 hover:to-indigo-700 text-white border border-violet-400/30"
                        : "bg-card text-[#1E293B] border border-slate-200 hover:border-violet-300 hover:bg-violet-50"
                    }`}
                  >
                    {activeAction === "TSforge" && busy ? (
                      <Loader2 className="w-5 h-5 animate-spin ml-2" />
                    ) : null}
                    TSforge (الكل)
                  </Button>
                </motion.div>
              </TooltipTrigger>
              <TooltipContent side="bottom" className={`text-sm max-w-xs text-center ${isDark ? "bg-slate-800 border border-violet-500/30 text-cyan-200" : "bg-white border border-slate-200 text-slate-700"}`}>
                تفعيل ويندوز وأوفيس عبر TSforge - يدعم إصدارات Windows 10/11 حتى 2026 وإصدارات Server
              </TooltipContent>
            </Tooltip>

            <Tooltip>
              <TooltipTrigger asChild>
                <motion.div whileHover={{ scale: 1.02 }} whileTap={{ scale: 0.97 }}>
                  <Button
                    onClick={() => requestActivation({ label: "Online KMS", sub: "", kind: "kms", name: "Online KMS" }, "onlinekms")}
                    disabled={busy}
                    className={`w-full rounded-lg py-6 font-semibold transition-all duration-300 ${
                      isDark
                        ? "bg-gradient-to-br from-cyan-600 to-teal-800 hover:from-cyan-500 hover:to-teal-700 text-white border border-cyan-400/30"
                        : "bg-card text-[#1E293B] border border-slate-200 hover:border-cyan-300 hover:bg-cyan-50"
                    }`}
                  >
                    {activeAction === "Online KMS" && busy ? (
                      <Loader2 className="w-5 h-5 animate-spin ml-2" />
                    ) : null}
                    Online KMS
                  </Button>
                </motion.div>
              </TooltipTrigger>
              <TooltipContent side="bottom" className={`text-sm max-w-xs text-center ${isDark ? "bg-slate-800 border border-cyan-500/30 text-cyan-200" : "bg-white border border-slate-200 text-slate-700"}`}>
                تفعيل أوفيس عبر KMS - يحتاج اتصال بالإنترنت، التفعيل يستمر 180 يوم ويتم التجديد تلقائياً
              </TooltipContent>
            </Tooltip>
          </div>

          {/* Change Windows Edition (7.6) */}
          <div className="mt-4">
            <Button
              onClick={() => setEditionDialog(true)}
              disabled={busy}
              className={`w-full rounded-lg py-5 font-semibold transition-all duration-300 border ${
                isDark
                  ? "bg-slate-800/60 hover:bg-slate-800 text-cyan-200 border-cyan-500/20"
                  : "bg-card text-[#1E293B] border border-slate-200 hover:border-slate-300 hover:bg-slate-50"
              }`}
            >
              <ArrowLeftRight className="w-5 h-5 ml-2" />
              تغيير إصدار Windows
            </Button>
          </div>
        </motion.div>

        {/* Logs toggle button */}
        <div className="mt-8 flex flex-wrap items-center justify-center gap-3">
          {logs.length > 0 && (
            <motion.button
              onClick={() => setShowLogs(!showLogs)}
              className={`px-6 py-2 rounded-lg text-sm font-medium transition-all duration-300 ${
                isDark ? "bg-cyan-900/30 hover:bg-cyan-900/50 border border-cyan-500/30 text-cyan-300"
                  : "bg-slate-100 hover:bg-slate-200 border border-slate-200 text-slate-600"
              }`}
              variants={itemVariants}
            >
              {showLogs ? "إخفاء السجل" : "عرض السجل"}
            </motion.button>
          )}
          <motion.button
            onClick={handleExportReport}
            className={`px-6 py-2 rounded-lg text-sm font-medium flex items-center gap-2 transition-all duration-300 ${
              isDark ? "bg-cyan-900/30 hover:bg-cyan-900/50 border border-cyan-500/30 text-cyan-300"
                : "bg-slate-100 hover:bg-slate-200 border border-slate-200 text-slate-600"
            }`}
            variants={itemVariants}
          >
            <FileDown className="w-4 h-4" />
            حفظ التقرير التشخيصي
          </motion.button>
        </div>

        {/* Logs display */}
        {showLogs && (
          <div className={`mt-6 w-full max-w-2xl rounded-lg p-4 backdrop-blur-sm ${isDark ? "bg-black/40 border border-cyan-500/20" : "bg-white border border-slate-200"}`}>
            <LogViewer logs={logs} isDark={isDark} maxHeightClass="max-h-48" />
          </div>
        )}
      </motion.div>

      {/* Status Dialog */}
      <StatusDialog
        open={statusDialog}
        onOpenChange={setStatusDialog}
        status={status}
        statusLoading={statusLoading}
        statusResult={statusResult}
        logs={logs}
        isDark={isDark}
      />

      {/* Edition Dialog (7.6) */}
      <EditionDialog
        open={editionDialog}
        onOpenChange={setEditionDialog}
        isDark={isDark}
        onLog={addLog}
      />

      {/* Confirm Dialog (7.3) */}
      <ConfirmDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        title={pendingAction ? `تأكيد ${pendingAction.action.label}` : "تأكيد العملية"}
        description={
          pendingAction
            ? `سيتم تنفيذ «${pendingAction.action.label}» على نظامك — يُنزَّل سكربت التفعيل من المصدر الرسمي massgrave.dev ويُنفَّذ بصلاحيات المسؤول. قد تستغرق العملية عدة دقائق. هل تريد المتابعة؟`
            : ""
        }
        confirmLabel="متابعة التنفيذ"
        loading={busy}
        onConfirm={confirmAndRun}
        isDark={isDark}
      />

      {/* Pin adoption dialog (4.1 self-healing) */}
      <ConfirmDialog
        open={pinAdoption !== null}
        onOpenChange={(v) => !adopting && setPinAdoption(v ? pinAdoption : null)}
        title="اعتماد إصدار جديد من سكربت التفعيل"
        description={
          pinAdoption
            ? `صدر إصدار جديد من سكربت التفعيل الرسمي (${pinAdoption.from} → ${pinAdoption.to}). سيتم تنزيله من المصدر الرسمي وحساب بصمته وتسجيلها محليًا مع وقت الاعتماد. الاعتماد قرارك — بلا موافقة لن يُنفذ أي محتوى جديد.`
            : ""
        }
        confirmLabel="اعتماد الإصدار الجديد"
        loading={adopting}
        onConfirm={handleAdoptPin}
        isDark={isDark}
      />

      {/* Confetti celebration overlay */}
      {confettiOrigin && celebratingKey && (
        <Confetti
          originX={confettiOrigin.x}
          originY={confettiOrigin.y}
          onComplete={() => setConfettiOrigin(null)}
        />
      )}

      {/* Shimmer keyframes */}
      <style>{`
        @keyframes shimmer {
          0% { transform: translateX(-100%); }
          100% { transform: translateX(100%); }
        }
      `}</style>
    </div>
  );
}
