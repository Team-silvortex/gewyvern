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

// Candidate v1.1 review shelf. Keeping this separate from the positional v1
// baseline makes the new copy easy to audit without changing legacy pack input.
const expandedCoreUi = {
  "pt-BR": ["Seguir o navegador", "Instale pacotes verificados da mesma origem ou importe um pacote JSON local.", "Atualizar catálogo", "Importar JSON", "Instalados", "Downloads disponíveis", "Nenhum pacote para download está publicado no momento.", "Nenhum pacote de idioma adicional instalado.", "Exportar", "Seguir o sistema", "Dia", "Noite"],
  it: ["Segui il browser", "Installa pacchetti verificati della stessa origine oppure importa un pacchetto JSON locale.", "Aggiorna catalogo", "Importa JSON", "Installati", "Download disponibili", "Al momento non sono pubblicati pacchetti scaricabili.", "Nessun pacchetto lingua aggiuntivo installato.", "Esporta", "Segui il sistema", "Giorno", "Notte"],
  ru: ["Следовать настройкам браузера", "Установите проверенные пакеты из того же источника или импортируйте локальный пакет JSON.", "Обновить каталог", "Импортировать JSON", "Установленные", "Доступные загрузки", "Сейчас нет опубликованных пакетов для загрузки.", "Дополнительные языковые пакеты не установлены.", "Экспортировать", "Как в системе", "День", "Ночь"],
  ar: ["اتباع المتصفح", "ثبّت حزمًا موثّقة من المصدر نفسه أو استورد حزمة JSON محلية.", "تحديث الكتالوج", "استيراد JSON", "المثبّتة", "التنزيلات المتاحة", "لا توجد حزم قابلة للتنزيل منشورة حاليًا.", "لا توجد حزم لغات إضافية مثبّتة.", "تصدير", "اتباع النظام", "نهاري", "ليلي"],
  hi: ["ब्राउज़र के अनुसार", "सत्यापित समान-मूल पैक इंस्टॉल करें या स्थानीय JSON पैक आयात करें।", "कैटलॉग रीफ़्रेश करें", "JSON आयात करें", "इंस्टॉल किए गए", "उपलब्ध डाउनलोड", "फ़िलहाल कोई डाउनलोड करने योग्य पैक प्रकाशित नहीं है।", "कोई अतिरिक्त भाषा पैक इंस्टॉल नहीं है।", "निर्यात करें", "सिस्टम के अनुसार", "दिन", "रात"],
  bn: ["ব্রাউজার অনুসরণ করুন", "একই উৎসের যাচাইকৃত প্যাক ইনস্টল করুন অথবা স্থানীয় JSON প্যাক আমদানি করুন।", "ক্যাটালগ রিফ্রেশ করুন", "JSON আমদানি করুন", "ইনস্টল করা", "উপলভ্য ডাউনলোড", "বর্তমানে ডাউনলোডযোগ্য কোনো প্যাক প্রকাশিত নেই।", "কোনো অতিরিক্ত ভাষা প্যাক ইনস্টল করা নেই।", "রপ্তানি করুন", "সিস্টেম অনুসরণ করুন", "দিন", "রাত"],
  id: ["Ikuti browser", "Pasang paket terverifikasi dari origin yang sama atau impor paket JSON lokal.", "Segarkan katalog", "Impor JSON", "Terpasang", "Unduhan tersedia", "Saat ini tidak ada paket unduhan yang dipublikasikan.", "Tidak ada paket bahasa tambahan yang terpasang.", "Ekspor", "Ikuti sistem", "Siang", "Malam"],
  ms: ["Ikut pelayar", "Pasang pek disahkan daripada asal yang sama atau import pek JSON setempat.", "Segar semula katalog", "Import JSON", "Dipasang", "Muat turun tersedia", "Tiada pek boleh muat turun diterbitkan pada masa ini.", "Tiada pek bahasa tambahan dipasang.", "Eksport", "Ikut sistem", "Siang", "Malam"],
  th: ["ตามเบราว์เซอร์", "ติดตั้งแพ็กที่ตรวจสอบแล้วจากต้นทางเดียวกัน หรือนำเข้าแพ็ก JSON ในเครื่อง", "รีเฟรชแค็ตตาล็อก", "นำเข้า JSON", "ติดตั้งแล้ว", "ดาวน์โหลดที่พร้อมใช้งาน", "ขณะนี้ยังไม่มีแพ็กที่เผยแพร่ให้ดาวน์โหลด", "ยังไม่ได้ติดตั้งแพ็กภาษาเพิ่มเติม", "ส่งออก", "ตามระบบ", "กลางวัน", "กลางคืน"],
  vi: ["Theo trình duyệt", "Cài đặt gói đã xác minh từ cùng nguồn hoặc nhập gói JSON cục bộ.", "Làm mới danh mục", "Nhập JSON", "Đã cài đặt", "Bản tải xuống khả dụng", "Hiện chưa có gói tải xuống nào được phát hành.", "Chưa cài đặt gói ngôn ngữ bổ sung.", "Xuất", "Theo hệ thống", "Ban ngày", "Ban đêm"],
  tr: ["Tarayıcıyı izle", "Doğrulanmış aynı kaynaklı paketleri yükleyin veya yerel bir JSON paketi içe aktarın.", "Kataloğu yenile", "JSON içe aktar", "Yüklü", "Kullanılabilir indirmeler", "Şu anda yayımlanmış indirilebilir paket yok.", "Ek dil paketi yüklü değil.", "Dışa aktar", "Sistemi izle", "Gündüz", "Gece"],
  pl: ["Zgodnie z przeglądarką", "Zainstaluj zweryfikowane pakiety z tego samego źródła lub zaimportuj lokalny pakiet JSON.", "Odśwież katalog", "Importuj JSON", "Zainstalowane", "Dostępne pliki", "Obecnie nie opublikowano żadnych pakietów do pobrania.", "Nie zainstalowano dodatkowych pakietów językowych.", "Eksportuj", "Zgodnie z systemem", "Dzień", "Noc"],
  nl: ["Browser volgen", "Installeer geverifieerde pakketten van dezelfde oorsprong of importeer een lokaal JSON-pakket.", "Catalogus vernieuwen", "JSON importeren", "Geïnstalleerd", "Beschikbare downloads", "Er zijn momenteel geen downloadbare pakketten gepubliceerd.", "Er zijn geen extra taalpakketten geïnstalleerd.", "Exporteren", "Systeem volgen", "Dag", "Nacht"],
  uk: ["Як у браузері", "Установіть перевірені пакети з того самого джерела або імпортуйте локальний пакет JSON.", "Оновити каталог", "Імпортувати JSON", "Установлені", "Доступні завантаження", "Наразі немає опублікованих пакетів для завантаження.", "Додаткові мовні пакети не встановлено.", "Експортувати", "Як у системі", "День", "Ніч"],
  cs: ["Podle prohlížeče", "Nainstalujte ověřené balíčky ze stejného zdroje nebo importujte místní balíček JSON.", "Obnovit katalog", "Importovat JSON", "Nainstalované", "Dostupná stažení", "Momentálně nejsou publikovány žádné balíčky ke stažení.", "Nejsou nainstalovány žádné další jazykové balíčky.", "Exportovat", "Podle systému", "Den", "Noc"],
  sv: ["Följ webbläsaren", "Installera verifierade paket från samma ursprung eller importera ett lokalt JSON-paket.", "Uppdatera katalog", "Importera JSON", "Installerade", "Tillgängliga hämtningar", "Inga hämtningsbara paket är publicerade just nu.", "Inga ytterligare språkpaket är installerade.", "Exportera", "Följ systemet", "Dag", "Natt"],
  da: ["Følg browseren", "Installer verificerede pakker fra samme oprindelse, eller importér en lokal JSON-pakke.", "Opdater katalog", "Importér JSON", "Installerede", "Tilgængelige downloads", "Der er i øjeblikket ingen offentliggjorte pakker til download.", "Der er ikke installeret yderligere sprogpakker.", "Eksportér", "Følg systemet", "Dag", "Nat"],
  no: ["Følg nettleseren", "Installer verifiserte pakker fra samme opprinnelse, eller importer en lokal JSON-pakke.", "Oppdater katalog", "Importer JSON", "Installert", "Tilgjengelige nedlastinger", "Det er for øyeblikket ingen publiserte pakker for nedlasting.", "Ingen ekstra språkpakker er installert.", "Eksporter", "Følg systemet", "Dag", "Natt"],
  fi: ["Seuraa selainta", "Asenna vahvistettuja paketteja samasta alkuperästä tai tuo paikallinen JSON-paketti.", "Päivitä luettelo", "Tuo JSON", "Asennetut", "Saatavilla olevat lataukset", "Julkaistuja ladattavia paketteja ei ole tällä hetkellä.", "Lisäkielipaketteja ei ole asennettu.", "Vie", "Seuraa järjestelmää", "Päivä", "Yö"],
  el: ["Ακολούθηση προγράμματος περιήγησης", "Εγκαταστήστε επαληθευμένα πακέτα από την ίδια προέλευση ή εισαγάγετε ένα τοπικό πακέτο JSON.", "Ανανέωση καταλόγου", "Εισαγωγή JSON", "Εγκατεστημένα", "Διαθέσιμες λήψεις", "Δεν υπάρχουν δημοσιευμένα πακέτα για λήψη αυτήν τη στιγμή.", "Δεν έχουν εγκατασταθεί πρόσθετα πακέτα γλώσσας.", "Εξαγωγή", "Ακολούθηση συστήματος", "Ημέρα", "Νύχτα"],
  he: ["לפי הדפדפן", "התקינו חבילות מאומתות מאותו מקור או ייבאו חבילת JSON מקומית.", "רענון הקטלוג", "ייבוא JSON", "מותקנות", "הורדות זמינות", "אין כרגע חבילות שפורסמו להורדה.", "לא מותקנות חבילות שפה נוספות.", "ייצוא", "לפי המערכת", "יום", "לילה"],
  fa: ["مطابق مرورگر", "بسته‌های تأییدشده از همان مبدأ را نصب کنید یا یک بسته JSON محلی وارد کنید.", "تازه‌سازی فهرست", "درون‌ریزی JSON", "نصب‌شده", "دریافت‌های موجود", "در حال حاضر هیچ بسته‌ای برای دریافت منتشر نشده است.", "هیچ بستهٔ زبان دیگری نصب نشده است.", "برون‌ریزی", "مطابق سیستم", "روز", "شب"],
};

const expandedCoreUiFieldCount = 12;
const definitionLocales = new Set(definitions.map(([locale]) => locale));
if (Object.keys(expandedCoreUi).length !== definitions.length
    || !Object.keys(expandedCoreUi).every((locale) => definitionLocales.has(locale))) {
  throw new Error("expanded core UI locale roster drifted from official definitions");
}

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
  const expanded = expandedCoreUi[locale];
  if (!expanded || expanded.length !== expandedCoreUiFieldCount) {
    throw new Error(`invalid expanded core UI definition for ${locale}`);
  }
  const [
    languageAuto,
    packSubcopy,
    packRefresh,
    packImport,
    packInstalledTitle,
    packCatalogTitle,
    packCatalogEmpty,
    packNoneInstalled,
    packExport,
    themeAuto,
    themeLight,
    themeDark,
  ] = expanded;
  return {
    schema: "leserpent.language-pack/v1",
    locale,
    name,
    nativeName,
    version: "1.1.0",
    author: "Leserpent maintainers",
    direction: rtlLocales.has(locale) ? "rtl" : "ltr",
    coverage: "core-ui",
    translations: {
      hero: { title, subcopy },
      language: { label: language, auto: languageAuto },
      languagePacks: {
        title: packs,
        subcopy: packSubcopy,
        refresh: packRefresh,
        import: packImport,
        installedTitle: packInstalledTitle,
        catalogTitle: packCatalogTitle,
        catalogEmpty: packCatalogEmpty,
        noneInstalled: packNoneInstalled,
        install,
        installedLabel: install,
        download,
        export: packExport,
        remove,
        coverageCore,
      },
      theme: { label: theme, auto: themeAuto, light: themeLight, dark: themeDark },
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
