# MAS Activator - Rust + React Modern UI

## نسخة عصرية احترافية من تطبيق تفعيل Windows و Office

### المميزات الرئيسية:
- ✨ واجهة عصرية (Modern UI) مع خلفية شبكية متحركة
- 🎨 تأثيرات Neon Glow وحركات سلسة (Framer Motion)
- 🌙 ثيم داكن احترافي (Dark Cyberpunk Theme)
- 🔧 Backend مكتوب بـ Rust (Tauri)
- 📱 واجهة عربية كاملة (RTL)
- 🎯 تفعيل Windows و Office بسهولة

---

## المتطلبات:

### Windows:
- Node.js 18+
- Rust 1.77+
- Visual Studio Build Tools أو MinGW

### Linux:
```bash
sudo apt-get install -y \
  build-essential \
  pkg-config \
  libssl-dev \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev
```

---

## خطوات البناء:

### 1. استخراج الملفات:
```bash
tar -xzf mas-activator-complete.tar.gz
cd mas-activator-rust
```

### 2. تثبيت المتطلبات:
```bash
# تثبيت Node dependencies
pnpm install

# تثبيت Rust و Cargo (إذا لم تكن مثبتة)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# تثبيت Tauri CLI
npm install -g @tauri-apps/cli@latest
```

### 3. بناء التطبيق:
```bash
# بناء الواجهة الأمامية
pnpm build

# بناء تطبيق Tauri (سينتج ملف EXE)
pnpm tauri build
```

### 4. الملف الناتج:
```
src-tauri/target/release/MASActivator.exe
```

---

## هيكل المشروع:

```
mas-activator-rust/
├── client/                    # React Frontend
│   ├── src/
│   │   ├── pages/
│   │   │   └── Home.tsx      # الصفحة الرئيسية
│   │   ├── components/       # مكونات UI
│   │   ├── App.tsx           # تطبيق رئيسي
│   │   └── index.css         # الأنماط والخطوط
│   └── package.json
├── src-tauri/                # Rust Backend (Tauri)
│   ├── src/
│   │   ├── lib.rs           # منطق التفعيل
│   │   └── main.rs          # نقطة الدخول
│   ├── tauri.conf.json      # إعدادات Tauri
│   └── Cargo.toml           # Rust dependencies
└── package.json             # Node.js configuration
```

---

## الملفات الرئيسية:

### Frontend (React + Tailwind):
- `client/src/pages/Home.tsx` - الواجهة الرئيسية مع جميع الأزرار
- `client/src/index.css` - الأنماط والخلفية الشبكية المتحركة
- `client/src/App.tsx` - إعدادات التطبيق والثيم

### Backend (Rust):
- `src-tauri/src/lib.rs` - أوامر التفعيل (activate_windows, activate_office, etc.)
- `src-tauri/tauri.conf.json` - إعدادات النافذة والتطبيق

---

## الأوامر المتاحة:

### في المشروع:
```bash
pnpm dev          # تشغيل في وضع التطوير
pnpm build        # بناء الواجهة الأمامية
pnpm check        # فحص TypeScript
pnpm format       # تنسيق الكود
pnpm tauri build  # بناء التطبيق النهائي
```

---

## معلومات المطور:

- **الاسم**: يزيد يحيى
- **الإصدار**: 2.0.0
- **الترخيص**: MIT

---

## ملاحظات مهمة:

1. **الواجهة الحالية**: تحتوي على محاكاة (Mock) للتفعيل. يمكن ربطها بـ APIs حقيقية.
2. **الأمان**: تأكد من تشغيل التطبيق بصلاحيات مسؤول على Windows.
3. **الأداء**: الخلفية الشبكية المتحركة قد تستهلك موارد إضافية على الأجهزة الضعيفة.

---

## الدعم والمساعدة:

للمزيد من المعلومات:
- [Tauri Documentation](https://tauri.app)
- [React Documentation](https://react.dev)
- [Tailwind CSS](https://tailwindcss.com)

