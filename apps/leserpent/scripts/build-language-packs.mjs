import { createHash } from "node:crypto";
import { mkdir, readdir, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "Leserpent", "wwwroot", "language-packs");
const rtlLocales = new Set(["ar", "he", "fa"]);

// locale, English name, native name, then core-shell translations.
const definitions = [
  ["pt-BR", "Portuguese (Brazil)", "Português (Brasil)", "Painel do plano de controle", "Uma visão leve da frota para vários runtimes gewyvern próximos.", "Idioma", "Pacotes de idioma", "Instalar", "core UI", "Baixar", "Remover", "Tema", "Visão geral", "Runtimes", "Registrar", "Persistência", "Sessões", "Painel filho", "Abrir todos", "Fechar todos"],
  ["it", "Italian", "Italiano", "Dashboard del piano di controllo", "Una vista leggera della flotta per più runtime gewyvern vicini.", "Lingua", "Pacchetti lingua", "Installa", "core UI", "Scarica", "Rimuovi", "Tema", "Panoramica", "Runtime", "Registra", "Persistenza", "Sessioni", "Pannello figlio", "Apri tutti", "Chiudi tutti"],
  ["ru", "Russian", "Русский", "Панель управления", "Лёгкое представление флота для нескольких ближайших runtime gewyvern.", "Язык", "Языковые пакеты", "Установить", "ядро интерфейса", "Скачать", "Удалить", "Тема", "Обзор", "Runtime", "Регистрация", "Хранилище", "Сеансы", "Дочерняя панель", "Открыть все", "Закрыть все"],
  ["ar", "Arabic", "العربية", "لوحة مستوى التحكم", "عرض خفيف للأسطول لعدة بيئات تشغيل gewyvern قريبة.", "اللغة", "حزم اللغات", "تثبيت", "واجهة أساسية", "تنزيل", "إزالة", "السمة", "نظرة عامة", "بيئات التشغيل", "تسجيل", "الاستمرارية", "الجلسات", "اللوحة الفرعية", "فتح الكل", "إغلاق الكل"],
  ["hi", "Hindi", "हिन्दी", "कंट्रोल प्लेन डैशबोर्ड", "कई निकटवर्ती gewyvern रनटाइम के लिए हल्का फ़्लीट दृश्य।", "भाषा", "भाषा पैक", "इंस्टॉल", "कोर यूआई", "डाउनलोड", "हटाएँ", "थीम", "अवलोकन", "रनटाइम", "पंजीकरण", "स्थायित्व", "सत्र", "चाइल्ड पैनल", "सभी खोलें", "सभी बंद करें"],
  ["bn", "Bengali", "বাংলা", "কন্ট্রোল প্লেন ড্যাশবোর্ড", "একাধিক কাছাকাছি gewyvern রানটাইমের জন্য হালকা ফ্লিট ভিউ।", "ভাষা", "ভাষা প্যাক", "ইনস্টল", "কোর UI", "ডাউনলোড", "সরান", "থিম", "সারসংক্ষেপ", "রানটাইম", "নিবন্ধন", "স্থায়িত্ব", "সেশন", "চাইল্ড প্যানেল", "সব খুলুন", "সব বন্ধ করুন"],
  ["id", "Indonesian", "Bahasa Indonesia", "Dasbor bidang kontrol", "Tampilan armada ringan untuk beberapa runtime gewyvern terdekat.", "Bahasa", "Paket bahasa", "Pasang", "UI inti", "Unduh", "Hapus", "Tema", "Ringkasan", "Runtime", "Daftarkan", "Persistensi", "Sesi", "Panel anak", "Buka semua", "Tutup semua"],
  ["ms", "Malay", "Bahasa Melayu", "Papan pemuka satah kawalan", "Paparan armada ringan untuk beberapa runtime gewyvern berdekatan.", "Bahasa", "Pek bahasa", "Pasang", "UI teras", "Muat turun", "Buang", "Tema", "Gambaran keseluruhan", "Runtime", "Daftar", "Kekekalan", "Sesi", "Panel anak", "Buka semua", "Tutup semua"],
  ["th", "Thai", "ไทย", "แดชบอร์ดระนาบควบคุม", "มุมมองฟลีตแบบเบาสำหรับ gewyvern runtime หลายตัวที่อยู่ใกล้กัน", "ภาษา", "แพ็กภาษา", "ติดตั้ง", "โหมดหลัก", "ดาวน์โหลด", "นำออก", "ธีม", "ภาพรวม", "รันไทม์", "ลงทะเบียน", "การคงอยู่", "เซสชัน", "แผงย่อย", "เปิดทั้งหมด", "ปิดทั้งหมด"],
  ["vi", "Vietnamese", "Tiếng Việt", "Bảng điều khiển mặt phẳng điều khiển", "Chế độ xem đội nhẹ cho nhiều runtime gewyvern ở gần.", "Ngôn ngữ", "Gói ngôn ngữ", "Cài đặt", "Giao diện cốt lõi", "Tải xuống", "Gỡ bỏ", "Chủ đề", "Tổng quan", "Runtime", "Đăng ký", "Lưu trữ", "Phiên", "Bảng con", "Mở tất cả", "Đóng tất cả"],
  ["tr", "Turkish", "Türkçe", "Kontrol düzlemi panosu", "Yakındaki birden çok gewyvern çalışma zamanı için hafif filo görünümü.", "Dil", "Dil paketleri", "Yükle", "çekirdek arayüz", "İndir", "Kaldır", "Tema", "Genel bakış", "Çalışma zamanları", "Kaydet", "Kalıcılık", "Oturumlar", "Alt panel", "Tümünü aç", "Tümünü kapat"],
  ["pl", "Polish", "Polski", "Panel płaszczyzny sterowania", "Lekki widok floty dla wielu pobliskich środowisk wykonawczych gewyvern.", "Język", "Pakiety językowe", "Zainstaluj", "interfejs rdzenia", "Pobierz", "Usuń", "Motyw", "Przegląd", "Runtime", "Zarejestruj", "Trwałość", "Sesje", "Panel podrzędny", "Otwórz wszystkie", "Zamknij wszystkie"],
  ["nl", "Dutch", "Nederlands", "Dashboard voor het besturingsvlak", "Een lichte vlootweergave voor meerdere gewyvern-runtimes in de buurt.", "Taal", "Taalpakketten", "Installeren", "kern-UI", "Downloaden", "Verwijderen", "Thema", "Overzicht", "Runtimes", "Registreren", "Persistentie", "Sessies", "Onderliggend paneel", "Alles openen", "Alles sluiten"],
  ["uk", "Ukrainian", "Українська", "Панель площини керування", "Легкий огляд флоту для кількох сусідніх runtime gewyvern.", "Мова", "Мовні пакети", "Установити", "Ядро інтерфейсу", "Завантажити", "Видалити", "Тема", "Огляд", "Runtime", "Реєстрація", "Сховище", "Сеанси", "Дочірня панель", "Відкрити всі", "Закрити всі"],
  ["cs", "Czech", "Čeština", "Řídicí panel", "Lehký pohled na flotilu několika blízkých runtime gewyvern.", "Jazyk", "Jazykové balíčky", "Nainstalovat", "jádro UI", "Stáhnout", "Odebrat", "Motiv", "Přehled", "Runtime", "Registrovat", "Perzistence", "Relace", "Podřízený panel", "Otevřít vše", "Zavřít vše"],
  ["sv", "Swedish", "Svenska", "Kontrollplanspanel", "En lätt flottvy för flera närliggande gewyvern-körmiljöer.", "Språk", "Språkpaket", "Installera", "kärnagränssnitt", "Hämta", "Ta bort", "Tema", "Översikt", "Körmiljöer", "Registrera", "Beständighet", "Sessioner", "Underpanel", "Öppna alla", "Stäng alla"],
  ["da", "Danish", "Dansk", "Kontrolplanspanel", "En let flådevisning til flere nærliggende gewyvern-kørsler.", "Sprog", "Sprogpakker", "Installer", "kerne-UI", "Download", "Fjern", "Tema", "Oversigt", "Kørsler", "Registrer", "Persistens", "Sessioner", "Underpanel", "Åbn alle", "Luk alle"],
  ["no", "Norwegian", "Norsk", "Kontrollplandashboard", "En lett flåtevisning for flere gewyvern-kjøretider i nærheten.", "Språk", "Språkpakker", "Installer", "kjerne-UI", "Last ned", "Fjern", "Tema", "Oversikt", "Kjøretider", "Registrer", "Persistens", "Økter", "Underpanel", "Åpne alle", "Lukk alle"],
  ["fi", "Finnish", "Suomi", "Ohjaustason koontinäyttö", "Kevyt laivastonäkymä useille lähellä oleville gewyvern-ajoympäristöille.", "Kieli", "Kielipaketit", "Asenna", "ydin-käyttöliittymä", "Lataa", "Poista", "Teema", "Yleiskatsaus", "Ajoympäristöt", "Rekisteröi", "Pysyvyys", "Istunnot", "Alipaneeli", "Avaa kaikki", "Sulje kaikki"],
  ["el", "Greek", "Ελληνικά", "Πίνακας επιπέδου ελέγχου", "Μια ελαφριά προβολή στόλου για πολλά κοντινά runtime gewyvern.", "Γλώσσα", "Πακέτα γλώσσας", "Εγκατάσταση", "κεντρικό UI", "Λήψη", "Κατάργηση", "Θέμα", "Επισκόπηση", "Runtime", "Εγγραφή", "Μονιμότητα", "Συνεδρίες", "Θυγατρικός πίνακας", "Άνοιγμα όλων", "Κλείσιμο όλων"],
  ["he", "Hebrew", "עברית", "לוח מישור הבקרה", "תצוגת צי קלה עבור מספר סביבות gewyvern קרובות.", "שפה", "חבילות שפה", "התקנה", "UI ליבה", "הורדה", "הסרה", "ערכת נושא", "סקירה", "סביבות ריצה", "רישום", "התמדה", "הפעלות", "לוח משנה", "פתיחת הכול", "סגירת הכול"],
  ["fa", "Persian", "فارسی", "داشبورد صفحه کنترل", "نمای سبک ناوگان برای چند محیط اجرای نزدیک gewyvern.", "زبان", "بسته‌های زبان", "نصب", "هسته رابط کاربری", "دانلود", "حذف", "پوسته", "نمای کلی", "محیط‌های اجرا", "ثبت", "ماندگاری", "نشست‌ها", "پنل فرزند", "باز کردن همه", "بستن همه"],
];

function makePack(row) {
  if (row.length !== 20) throw new Error(`invalid language definition for ${row[0]}`);
  const [
    locale,
    name,
    nativeName,
    title,
    subcopy,
    language,
    packs,
    install,
    coverageCore,
    download,
    remove,
    theme,
    overview,
    runtimes,
    register,
    persistence,
    sessions,
    panel,
    openAll,
    closeAll,
  ] = row;
  return {
    schema: "leserpent.language-pack/v1",
    locale,
    name,
    nativeName,
    version: "1.0.0",
    author: "Leserpent maintainers",
    direction: rtlLocales.has(locale) ? "rtl" : "ltr",
    coverage: "core-ui",
    translations: {
      hero: { title, subcopy },
      language: { label: language },
      languagePacks: { title: packs, install, installedLabel: install, download, remove, coverageCore },
      theme: { label: theme },
      tabs: { overview, runtimes, register, persistence, sessions },
      runtimes: { workspaceTabs: { panel } },
      runtimePanel: { windows: { openAll, closeAll } },
    },
  };
}

await mkdir(root, { recursive: true });
for (const file of await readdir(root)) {
  if (file.endsWith(".json")) await rm(join(root, file));
}

const seen = new Set();
const packs = [];
for (const definition of definitions) {
  const pack = makePack(definition);
  if (seen.has(pack.locale)) throw new Error(`duplicate locale ${pack.locale}`);
  seen.add(pack.locale);
  const text = `${JSON.stringify(pack, null, 2)}\n`;
  const fileName = `${pack.locale}.json`;
  await writeFile(join(root, fileName), text, "utf8");
  packs.push({
    locale: pack.locale,
    name: pack.name,
    nativeName: pack.nativeName,
    version: pack.version,
    direction: pack.direction,
    coverage: pack.coverage,
    url: `/language-packs/${fileName}`,
    sha256: createHash("sha256").update(text).digest("hex"),
  });
}

const catalog = {
  schema: "leserpent.language-pack-catalog/v1",
  generatedAt: "2026-07-13T00:00:00Z",
  officialLocaleCount: 30,
  builtinLocaleCount: 8,
  downloadableLocaleCount: packs.length,
  packs,
};
await writeFile(join(root, "catalog.json"), `${JSON.stringify(catalog, null, 2)}\n`, "utf8");
