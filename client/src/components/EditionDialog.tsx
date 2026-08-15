import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Loader2, Settings2, RefreshCw, CheckCircle2, ShieldAlert, ArrowLeftRight, Play, Ban, ScrollText } from "lucide-react";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { LogViewer } from "@/components/ui/LogViewer";
import type { EditionChangeResult, EditionChangeStatus, EditionPreflightReport, EditionSnapshot } from "@/types";

interface EditionDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  isDark: boolean;
  onLog?: (message: string) => void;
}

const STATE_MESSAGES: Record<EditionChangeStatus, string> = {
  idle: "",
  preflight_ready: "المسار متاح. تغيير الإصدار لا يعني التفعيل تلقائيًا.",
  unsupported_path: "لا يدعم النظام مسار تغيير الإصدار المطلوب.",
  discovery_failed: "تعذر استكشاف مسارات تغيير الإصدار على هذا النظام.",
  settings_opened:
    "فُتحت إعدادات تنشيط Windows. أدخل مفتاحًا رسميًا داخل إعدادات النظام، ثم ارجع واضغط تحقق الآن.",
  pending_restart:
    "قد يتطلب Windows إعادة التشغيل لإتمام تغيير الإصدار. أعد التشغيل ثم افتح التطبيق وتحقق.",
  edition_changed_and_activated: "تغير إصدار Windows وتم التحقق من أنه مفعل.",
  edition_changed_needs_activation: "تغير إصدار Windows، لكن الإصدار الجديد ليس مفعلًا بعد.",
  edition_unchanged: "لم يتغير إصدار Windows حتى الآن.",
  verification_failed: "تعذر التحقق من نتيجة تغيير الإصدار؛ لم نعلن نجاحًا نهائيًا.",
  cancelled: "أُلغيت عملية تغيير الإصدار.",
  timed_out: "انتهت مهلة عملية تغيير الإصدار (30 دقيقة) وأُنهيت؛ لم نعلن نجاحًا.",
};

const EXEC_TIMEOUT_SECONDS = 30 * 60;

const EDITION_DESCRIPTIONS: Record<string, string> = {
  CoreSingleLanguage: "نسخة Home مقيدة بلغة نظام واحدة غير قابلة للتغيير — غالبًا للأجهزة الاقتصادية",
  Professional: "للمستخدمين المتقدمين والشركات الصغيرة: BitLocker وHyper-V وسطح المكتب البعيد وسياسات المجموعة",
  ProfessionalSingleLanguage: "كل ميزات Pro مع تقييد بلغة عرض واحدة فقط",
  ProfessionalEducation: "مبنية على Pro ومخصصة للبيئات المدرسية بإعدادات تعليمية افتراضية",
  ProfessionalWorkstation: "لمحطات العمل القوية: معالجات خوادم وذاكرة حتى 6TB ونظام ملفات ReFS",
  Enterprise: "للشركات الكبرى: أمان وإدارة متقدمة (Credential Guard وAppLocker) — تتطلب تراخيص مجمعة",
  Education: "نواة Enterprise بترخيص مخصص للمؤسسات التعليمية والجامعات",
  IoTEnterprise: "للأجهزة ذات الغرض الواحد (ATM ونقاط البيع) بدعم وتحديثات أمنية طويلة المدى LTSC",
  IoTEnterpriseK: "نسخة IoTEnterprise مخصصة للسوق الكوري الجنوبي حصرًا",
  ProfessionalCountrySpecific: "Pro لأسواق معينة بشروط تسعير وتراخيص إقليمية",
  ServerRdsh: "استضافة جلسات سطح المكتب البعيد لمستخدمين متعددين",
  CloudEdition: "نظام مقفل يعتمد على الحوسبة السحابية ومتجر مايكروسوفت حصرًا",
};

function formatElapsed(totalSeconds: number) {
  const m = Math.floor(totalSeconds / 60);
  const s = totalSeconds % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

export function EditionDialog({ open, onOpenChange, isDark, onLog }: EditionDialogProps) {
  const [preflight, setPreflight] = useState<EditionPreflightReport | null>(null);
  const [preflightLoading, setPreflightLoading] = useState(false);
  const [beforeSnapshot, setBeforeSnapshot] = useState<EditionSnapshot | null>(null);
  const [verifyLoading, setVerifyLoading] = useState(false);
  const [result, setResult] = useState<EditionChangeResult | null>(null);
  const [selectedTarget, setSelectedTarget] = useState<string | null>(null);
  const [confirmTarget, setConfirmTarget] = useState(false);
  const [executing, setExecuting] = useState(false);
  const [execStart, setExecStart] = useState<number | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [showLogs, setShowLogs] = useState(false);
  const [backendLogs, setBackendLogs] = useState<string[]>([]);

  const refreshBackendLogs = async () => {
    try {
      const all = await invoke<string[]>("get_logs");
      setBackendLogs(all.slice(-12));
    } catch {
      // تجاهل — السجل الخلفي غير متاح
    }
  };

  useEffect(() => {
    if (open) {
      refreshBackendLogs();
    }
  }, [open]);

  useEffect(() => {
    if (execStart === null) return;
    const t = setInterval(() => setElapsed(Math.floor((Date.now() - execStart) / 1000)), 1000);
    return () => clearInterval(t);
  }, [execStart]);

  const runPreflight = async () => {
    setPreflightLoading(true);
    setResult(null);
    setSelectedTarget(null);
    setConfirmTarget(false);
    onLog?.("🔍 جاري استكشاف مسارات تغيير الإصدار...");
    try {
      const report = await invoke<EditionPreflightReport>("edition_preflight");
      setPreflight(report);
      setBeforeSnapshot(report.current);
      onLog?.("✅ اكتمل استكشاف مسارات تغيير الإصدار");
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      onLog?.(`❌ ${msg}`);
      toast.error(`تعذر الاستكشاف: ${msg}`);
      setPreflight({ current: null, supported_targets: [], blocked_targets: [], checked_at: null, error: { kind: "discovery_failed", message: msg } });
    } finally {
      setPreflightLoading(false);
      refreshBackendLogs();
    }
  };

  useEffect(() => {
    if (open) {
      runPreflight();
    }
  }, [open]);

  const derivedStatus: EditionChangeStatus = preflightLoading
    ? "idle"
    : preflight?.error
    ? "discovery_failed"
    : preflight && preflight.supported_targets.length === 0
    ? "unsupported_path"
    : preflight
    ? "preflight_ready"
    : "idle";

  const canProceed = derivedStatus === "preflight_ready" && !verifyLoading && !executing;

  const handleOpenSettings = async () => {
    try {
      const message = await invoke<string>("open_activation_settings");
      onLog?.(`ℹ️ ${message}`);
      toast.info(message);
      setResult({ status: "settings_opened", before: beforeSnapshot, after: null, restart_required: false, checked_at: null, safe_message: message });
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      onLog?.(`❌ ${msg}`);
      toast.error(msg);
    }
  };

  const handleVerify = async () => {
    if (!beforeSnapshot) {
      toast.warning("لا توجد لقطة «قبل» — أعد الاستكشاف أولًا");
      return;
    }
    setVerifyLoading(true);
    onLog?.("🔍 جاري التحقق من نتيجة تغيير الإصدار...");
    try {
      const r = await invoke<EditionChangeResult>("verify_edition_change", { before: beforeSnapshot });
      setResult(r);
      if (r.after) {
        setBeforeSnapshot(r.after);
      }
      onLog?.(`[${r.status}] ${r.safe_message}`);
      if (r.status === "edition_changed_and_activated") {
        toast.success(r.safe_message);
      } else if (r.status === "verification_failed") {
        toast.error(r.safe_message);
      } else {
        toast.info(r.safe_message);
      }
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      onLog?.(`❌ ${msg}`);
      toast.error(msg);
    } finally {
      setVerifyLoading(false);
      refreshBackendLogs();
    }
  };

  const handleExecute = async () => {
    if (!selectedTarget || !beforeSnapshot) {
      toast.warning("اختر إصدارًا مستهدفًا أولًا");
      return;
    }
    setExecuting(true);
    setExecStart(Date.now());
    setElapsed(0);
    setResult(null);
    onLog?.(`⚙️ جاري تنفيذ التحويل إلى ${selectedTarget}...`);
    try {
      const r = await invoke<EditionChangeResult>("change_edition", { target: selectedTarget, before: beforeSnapshot });
      setResult(r);
      if (r.after) {
        setBeforeSnapshot(r.after);
      }
      onLog?.(`[${r.status}] ${r.safe_message}`);
      if (r.status === "edition_changed_and_activated") {
        toast.success(r.safe_message);
      } else if (r.status === "cancelled") {
        toast.info(r.safe_message);
      } else if (r.status === "timed_out" || r.status === "verification_failed") {
        toast.error(r.safe_message);
      } else {
        toast.info(r.safe_message);
      }
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      onLog?.(`❌ ${msg}`);
      toast.error(msg);
    } finally {
      setExecuting(false);
      setExecStart(null);
      setConfirmTarget(false);
      refreshBackendLogs();
    }
  };

  const handleCancelExec = async () => {
    try {
      await invoke("cancel_operation");
      toast.info("أُرسل طلب الإلغاء");
    } catch {
      // قد تكون العملية انتهت بالفعل
    }
  };

  const statusStyle = (s: EditionChangeStatus) => {
    switch (s) {
      case "preflight_ready":
        return isDark ? "bg-cyan-900/30 border border-cyan-500/30 text-cyan-200" : "bg-cyan-50 border border-cyan-200 text-cyan-800";
      case "unsupported_path":
      case "discovery_failed":
      case "verification_failed":
        return isDark ? "bg-red-900/30 border border-red-500/30 text-red-200" : "bg-red-50 border border-red-200 text-red-700";
      case "edition_changed_and_activated":
        return isDark ? "bg-green-900/30 border border-green-500/30 text-green-200" : "bg-green-50 border border-green-200 text-green-700";
      default:
        return isDark ? "bg-amber-900/30 border border-amber-500/30 text-amber-200" : "bg-amber-50 border border-amber-200 text-amber-800";
    }
  };

  return (
    <Dialog open={open} onOpenChange={(v) => !executing && !verifyLoading && onOpenChange(v)}>
      <DialogContent className={`rounded-xl max-w-2xl max-h-[85vh] overflow-y-auto ${isDark ? "bg-gradient-to-br from-slate-900 to-slate-800 border border-cyan-500/30" : "bg-white border border-slate-200"}`}>
        <DialogHeader>
          <DialogTitle className={`text-2xl flex items-center gap-2 ${isDark ? "text-cyan-300" : "text-[#1E293B]"}`}>
            <ArrowLeftRight className="w-6 h-6" />
            تغيير إصدار Windows
          </DialogTitle>
          <DialogDescription className={isDark ? "text-cyan-200/60" : "text-slate-500"}>
            تغيير الإصدار ≠ التفعيل أو إثبات الترخيص. ستُعرض النتيجة بثلاث طبقات: هل المسار مدعوم؟ هل تغيّر الإصدار فعلًا؟ وهل هو مفعل؟
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          {preflightLoading ? (
            <div className="flex items-center justify-center gap-3 p-8">
              <Loader2 className={`w-6 h-6 animate-spin ${isDark ? "text-cyan-400" : "text-[#4682B4]"}`} />
              <span className={isDark ? "text-cyan-200" : "text-slate-600"}>جاري استكشاف النظام...</span>
            </div>
          ) : (
            <>
              {/* Current system info */}
              {preflight?.current && (
                <div className={`rounded-lg p-4 space-y-2 ${isDark ? "bg-black/30 border border-cyan-500/20" : "bg-slate-50 border border-slate-200"}`}>
                  <p className={`text-xs font-semibold ${isDark ? "text-cyan-300/70" : "text-slate-500"}`}>النظام الحالي:</p>
                  <div className="grid grid-cols-2 gap-2 text-sm">
                    <span className={isDark ? "text-cyan-200/60" : "text-slate-500"}>المنتج</span>
                    <span className={isDark ? "text-cyan-100" : "text-slate-700"}>{preflight.current.product_name}</span>
                    <span className={isDark ? "text-cyan-200/60" : "text-slate-500"}>EditionID</span>
                    <span className={`font-mono ${isDark ? "text-cyan-100" : "text-slate-700"}`} dir="ltr">{preflight.current.edition_id}</span>
                    <span className={isDark ? "text-cyan-200/60" : "text-slate-500"}>البناء</span>
                    <span className={`font-mono ${isDark ? "text-cyan-100" : "text-slate-700"}`} dir="ltr">
                      {preflight.current.current_build}.{preflight.current.ubr}
                      {preflight.current.display_version ? ` (${preflight.current.display_version})` : ""}
                    </span>
                    <span className={isDark ? "text-cyan-200/60" : "text-slate-500"}>حالة الترخيص</span>
                    <span className={isDark ? "text-cyan-100" : "text-slate-700"}>
                      {preflight.current.windows_label || "غير معروفة"}
                    </span>
                  </div>
                </div>
              )}

              {/* Status banner */}
              {derivedStatus !== "idle" && (
                <div className={`rounded-lg p-3 text-sm ${statusStyle(derivedStatus)}`}>
                  {STATE_MESSAGES[derivedStatus]}
                  {preflight?.error && <p className="mt-1 text-xs opacity-80">{preflight.error.message}</p>}
                </div>
              )}

              {/* Edition picker */}
              {canProceed && preflight && preflight.supported_targets.length > 0 && (
                <div className="space-y-2">
                  <p className={`text-xs font-semibold ${isDark ? "text-cyan-300/70" : "text-slate-500"}`}>
                    اختر الإصدار الذي تريد التحويل إليه:
                  </p>
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                    {preflight.supported_targets.map((t) => {
                      const selected = selectedTarget === t;
                      return (
                        <Tooltip key={t}>
                          <TooltipTrigger asChild>
                            <button
                              onClick={() => {
                                setSelectedTarget(t);
                                setConfirmTarget(false);
                                setResult(null);
                              }}
                              className={`text-sm px-3 py-2 rounded-lg border transition-all duration-200 text-left ${
                                selected
                                  ? isDark
                                    ? "bg-green-900/50 border-green-500 text-green-200 ring-1 ring-green-400"
                                    : "bg-green-50 border-green-400 text-green-800 ring-1 ring-green-300"
                                  : isDark
                                  ? "bg-slate-800/60 border-slate-600/40 text-cyan-100 hover:border-cyan-500/40"
                                  : "bg-white border-slate-200 text-slate-700 hover:border-slate-300"
                              }`}
                              dir="ltr"
                            >
                              {selected ? "✓ " : ""}{t}
                            </button>
                          </TooltipTrigger>
                          <TooltipContent side="bottom" className="text-xs max-w-xs text-right">
                            {EDITION_DESCRIPTIONS[t] || `التحويل إلى ${t} — قد يتطلب إعادة تشغيل النظام`}
                          </TooltipContent>
                        </Tooltip>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* Blocked targets */}
              {preflight && preflight.blocked_targets.length > 0 && (
                <div className="space-y-1">
                  <p className={`text-xs font-semibold ${isDark ? "text-cyan-300/50" : "text-slate-400"}`}>وجهات محجوبة (غير معروضة كخيارات):</p>
                  <div className="flex flex-wrap gap-2">
                    {preflight.blocked_targets.map((t) => (
                      <Tooltip key={t}>
                        <TooltipTrigger asChild>
                          <span className={`text-xs px-2 py-1 rounded-full line-through cursor-help ${isDark ? "bg-slate-800/60 border border-slate-600/40 text-slate-400" : "bg-slate-100 border border-slate-200 text-slate-400"}`} dir="ltr">
                            {t}
                          </span>
                        </TooltipTrigger>
                        <TooltipContent side="bottom" className="text-xs max-w-xs text-right">
                          {EDITION_DESCRIPTIONS[t] || t}
                        </TooltipContent>
                      </Tooltip>
                    ))}
                  </div>
                </div>
              )}

              {/* Confirm panel */}
              {confirmTarget && selectedTarget && !executing && (
                <div className={`rounded-lg p-4 space-y-3 ${isDark ? "bg-amber-900/30 border border-amber-500/30" : "bg-amber-50 border border-amber-200"}`}>
                  <p className={`text-sm font-semibold flex items-center gap-2 ${isDark ? "text-amber-200" : "text-amber-800"}`}>
                    <ShieldAlert className="w-4 h-4 shrink-0" />
                    تأكيد التحويل إلى <span dir="ltr">{selectedTarget}</span>
                  </p>
                  <ul className={`text-xs space-y-1 ${isDark ? "text-amber-100/80" : "text-amber-800"}`}>
                    <li>• يُنفَّذ التغيير عبر أدوات Windows الرسمية (changepk / slmgr / DISM) بصمت.</li>
                    <li>• يستخدم التطبيق مفتاح الإصدار العام الذي يسترجعه من النظام نفسه — لا يُدخل مفتاحك الشخصي ولا يُخزَّن.</li>
                    <li>• العملية عالية التأثير وقد تتطلب إعادة تشغيل النظام.</li>
                    <li>• بعد التنفيذ سيتحقق التطبيق من النتيجة فعليًا ولا يعلن نجاحًا دون دليل.</li>
                  </ul>
                  <div className="flex gap-3">
                    <Button onClick={handleExecute} className={`flex-1 text-white ${isDark ? "bg-green-700 hover:bg-green-600" : "bg-green-600 hover:bg-green-700"}`}>
                      <Play className="w-4 h-4 ml-2" />
                      تنفيذ التحويل الآن
                    </Button>
                    <Button onClick={() => setConfirmTarget(false)} variant="outline" className="flex-1">
                      رجوع
                    </Button>
                  </div>
                </div>
              )}

              {/* Executing state */}
              {executing && (
                <div className={`rounded-lg p-4 space-y-2 ${isDark ? "bg-cyan-900/30 border border-cyan-500/30" : "bg-cyan-50 border border-cyan-200"}`}>
                  <p className={`text-sm font-semibold flex items-center gap-2 ${isDark ? "text-cyan-200" : "text-cyan-800"}`}>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    جاري تنفيذ التحويل إلى <span dir="ltr">{selectedTarget}</span> ...
                  </p>
                  <p className={`text-xs font-mono ${isDark ? "text-cyan-200/70" : "text-cyan-700"}`} dir="ltr">
                    {formatElapsed(elapsed)} / {formatElapsed(EXEC_TIMEOUT_SECONDS)}
                  </p>
                  <Button onClick={handleCancelExec} className={`${isDark ? "bg-red-700/70 hover:bg-red-700 text-white border border-red-400/30" : "bg-red-600 hover:bg-red-700 text-white"}`}>
                    <Ban className="w-4 h-4 ml-2" />
                    إلغاء العملية
                  </Button>
                </div>
              )}

              {/* Verify result */}
              {result && (
                <div className={`rounded-lg p-4 space-y-2 ${statusStyle(result.status)}`}>
                  <p className="text-sm font-semibold">{result.safe_message}</p>
                  {result.before && result.after && (
                    <p className="text-xs opacity-80 font-mono" dir="ltr">
                      {result.before.edition_id} → {result.after.edition_id}
                      {result.after.windows_label ? ` | ${result.after.windows_label}` : ""}
                    </p>
                  )}
                  {result.restart_required && (
                    <p className="text-xs font-semibold flex items-center gap-1">
                      <ShieldAlert className="w-3.5 h-3.5" /> يلزم إعادة التشغيل للتحقق النهائي
                    </p>
                  )}
                  {result.checked_at && <p className="text-xs opacity-70">وقت الفحص: {result.checked_at}</p>}
                </div>
              )}

              {/* Backend log details */}
              {showLogs && (
                <div className={`rounded-lg p-3 ${isDark ? "bg-black/40 border border-cyan-500/20" : "bg-slate-50 border border-slate-200"}`}>
                  <LogViewer logs={backendLogs} isDark={isDark} maxHeightClass="max-h-40" showLabel />
                </div>
              )}
            </>
          )}
        </div>

        <div className="flex flex-wrap gap-3 pt-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                onClick={() => setConfirmTarget(true)}
                disabled={!canProceed || !selectedTarget}
                className={`flex-1 min-w-[160px] text-white ${isDark ? "bg-green-700 hover:bg-green-600" : "bg-green-600 hover:bg-green-700"} disabled:opacity-50`}
              >
                <Play className="w-4 h-4 ml-2" />
                تنفيذ تغيير الإصدار
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="text-xs max-w-xs">
              ينفذ التحويل بصمت عبر أدوات Windows الرسمية بمفتاح الإصدار العام المسترجع من النظام — لا يُدخل مفتاحك الشخصي
            </TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button onClick={handleOpenSettings} disabled={!canProceed} className={`flex-1 min-w-[160px] text-white ${isDark ? "bg-cyan-600 hover:bg-cyan-700" : "bg-[#4682B4] hover:bg-[#3b75a5]"} disabled:opacity-50`}>
                <Settings2 className="w-4 h-4 ml-2" />
                فتح إعدادات تنشيط Windows
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="text-xs max-w-xs">
              يفتح صفحة التنشيط الرسمية لإدخال مفتاحك الشخصي بنفسك
            </TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button onClick={handleVerify} disabled={!canProceed || !beforeSnapshot} className={`flex-1 min-w-[120px] ${isDark ? "bg-slate-700 hover:bg-slate-600 text-slate-200 border border-slate-500/30" : "bg-slate-100 hover:bg-slate-200 text-slate-700 border border-slate-200"} disabled:opacity-50`} variant="outline">
                {verifyLoading ? <Loader2 className="w-4 h-4 ml-2 animate-spin" /> : <CheckCircle2 className="w-4 h-4 ml-2" />}
                تحقق الآن
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="text-xs max-w-xs">
              يعيد فحص EditionID وحالة الترخيص ويقارنها بلقطة ما قبل العملية
            </TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button onClick={runPreflight} disabled={preflightLoading || verifyLoading || executing} className={`min-w-[120px] ${isDark ? "bg-slate-700 hover:bg-slate-600 text-slate-200 border border-slate-500/30" : "bg-slate-100 hover:bg-slate-200 text-slate-700 border border-slate-200"} disabled:opacity-50`} variant="outline">
                <RefreshCw className={`w-4 h-4 ml-2 ${preflightLoading ? "animate-spin" : ""}`} />
                إعادة الاستكشاف
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="text-xs max-w-xs">
              يعيد فحص النظام والوجهات المدعومة من جديد
            </TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                onClick={() => {
                  setShowLogs(!showLogs);
                  if (!showLogs) refreshBackendLogs();
                }}
                className={`min-w-[120px] ${isDark ? "bg-slate-700 hover:bg-slate-600 text-slate-200 border border-slate-500/30" : "bg-slate-100 hover:bg-slate-200 text-slate-700 border border-slate-200"}`}
                variant="outline"
              >
                <ScrollText className="w-4 h-4 ml-2" />
                {showLogs ? "إخفاء تفاصيل السجل" : "عرض تفاصيل السجل"}
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="text-xs max-w-xs">
              يعرض السجل التقني للخلفية بما فيه تفاصيل أخطاء الاسترجاع والتنفيذ
            </TooltipContent>
          </Tooltip>
        </div>
      </DialogContent>
    </Dialog>
  );
}
