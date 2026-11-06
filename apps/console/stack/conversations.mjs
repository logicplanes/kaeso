// What the seeded organization's apps say to models, and what the models answer: support tickets for
// Support triage and questions for the Slack helper in Internal tools. The seed sends these prompts
// through the gateway and the mock provider answers them with the replies here, so the request log
// holds content that reads like a real company's. Everything is generated from fixed seeds, so the
// seed and the mock build the same list. Times are relative ("five days ago"), never dates: the
// public demo moves its data forward every night.

function random(seed) {
  let state = seed >>> 0 || 1;
  return () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) / 4294967296;
  };
}

const pick = (next, list) => list[Math.floor(next() * list.length)];
const between = (next, low, high) => low + Math.floor(next() * (high - low + 1));

export const TRIAGE_SYSTEM = `You triage customer emails for Northwind Outfitters, an online shop for outdoor gear that ships across Europe.

Read the email and answer with one JSON object and nothing else:
{
  "category": one of "shipping_delay", "damaged_item", "wrong_item", "return_request", "refund_status", "payment_issue", "account_access", "product_question", "cancellation", "warranty_claim", "invoice_request",
  "priority": "low" | "normal" | "high" | "urgent",
  "language": the ISO 639-1 code of the email,
  "sentiment": "negative" | "neutral" | "positive",
  "order_id": the order number (NW- followed by six digits) or null,
  "needs_human": true when a person must decide (refunds over 150 EUR, legal threats, safety issues, anything you are unsure about),
  "summary": one sentence in English, at most 25 words
}

Priority:
- urgent: the order is about to ship and the customer wants to change or cancel it, or a safety problem with a product.
- high: money was taken wrongly, a parcel is lost or more than five days late, or the customer needs the item by a date that is at risk.
- normal: damaged or wrong items, returns, warranty claims, account problems.
- low: questions before buying, invoices, feedback.

Never invent an order number. If the email mentions several orders, use the first one.`;

export const DRAFT_SYSTEM = `You write replies for Northwind Outfitters' customer care team. An agent reads every draft before it is sent.

Write in the customer's language. Be warm, specific and brief: at most 150 words. Use the customer's first name. Say what we have already done and what happens next, with a time frame. Never promise anything the policies below don't allow, and never ask for information the ticket already gives.

Policies:
- Returns: 30 days from delivery, free with the prepaid label from the account page. Refunds go out within 5 business days after the return reaches the warehouse.
- Damaged or wrong items: we send a replacement right away; the customer keeps or recycles the damaged item unless it is over 200 EUR.
- Late parcels: after 5 business days without a scan we open an investigation with the carrier; after 10 we send a replacement or refund.
- Warranty: 2 years on all products, 5 years on tents and backpacks. Zips, buckles and seams are covered; wear and tear is not.
- Orders can be changed or cancelled until they are packed, usually within 2 hours.
- Invoices with a VAT number can be generated from the account page; we also send them on request.

Sign as "Northwind Customer Care" (German: "Ihr Northwind-Kundenservice", French: "Le service client Northwind").`;

export const SLACK_SYSTEM = `You are Northie, the internal assistant in Northwind's Slack. Answer colleagues briefly and concretely, in the language of the question. Use Slack formatting (*bold*, bullet lists, \`code\`). If a question needs data you don't have, say what to check instead of guessing.`;

const PRODUCTS = [
  { name: "Fjell 2 tent", kind: "tent", price: 349, colors: ["moss green", "sand"] },
  { name: "Fjell 3 tent", kind: "tent", price: 429, colors: ["moss green", "rust"] },
  { name: "Alta 30L daypack", kind: "backpack", price: 119, colors: ["black", "ocean blue", "ochre"] },
  { name: "Alta 45L trekking pack", kind: "backpack", price: 189, colors: ["black", "forest green"] },
  { name: "Kestrel rain shell", kind: "jacket", price: 229, colors: ["slate", "signal red", "black"], sized: true },
  { name: "Brisk wind jacket", kind: "jacket", price: 129, colors: ["teal", "black"], sized: true },
  { name: "Torvik hiking boots", kind: "boots", price: 189, colors: ["brown", "grey"], sized: true, shoe: true },
  { name: "Ridge trail runners", kind: "shoes", price: 149, colors: ["black", "lime"], sized: true, shoe: true },
  { name: "Sarek down sleeping bag", kind: "sleeping bag", price: 279, colors: ["navy"] },
  { name: "Vinter merino base layer", kind: "base layer", price: 89, colors: ["charcoal", "heather blue"], sized: true },
  { name: "Lumen 400 headlamp", kind: "headlamp", price: 49, colors: ["orange"] },
  { name: "Glaze insulated bottle", kind: "bottle", price: 34, colors: ["white", "dusk purple"] },
  { name: "Pika trekking poles", kind: "poles", price: 99, colors: ["carbon"] },
  { name: "Halde fleece jacket", kind: "fleece", price: 109, colors: ["oat", "pine"], sized: true },
];
const SIZES = ["XS", "S", "M", "L", "XL"];
const SHOE_SIZES = ["38", "39", "40", "41", "42", "43", "44", "45"];

const PEOPLE = {
  en: {
    first: ["Emma", "Oliver", "Sophie", "James", "Grace", "Liam", "Hannah", "Daniel", "Chloe", "Thomas", "Megan", "Ryan", "Isla", "Ben", "Aisha", "Callum"],
    last: ["Clarke", "Walsh", "Patel", "Hughes", "Murphy", "Bennett", "O'Brien", "Turner", "Shaw", "Kaur", "Fletcher", "Doyle"],
    cities: ["Manchester", "Dublin", "Bristol", "Leeds", "Edinburgh", "Cork"],
  },
  de: {
    first: ["Lena", "Jonas", "Mia", "Felix", "Anna", "Lukas", "Sarah", "Moritz", "Julia", "Tobias", "Katharina", "Niklas"],
    last: ["Müller", "Schmidt", "Wagner", "Becker", "Hoffmann", "Schäfer", "Koch", "Richter", "Klein", "Wolf", "Neumann", "Brandt"],
    cities: ["München", "Hamburg", "Köln", "Leipzig", "Wien", "Zürich", "Freiburg"],
  },
  fr: {
    first: ["Camille", "Louis", "Manon", "Hugo", "Léa", "Théo", "Chloé", "Antoine", "Inès", "Julien", "Clara", "Mathis"],
    last: ["Martin", "Bernard", "Dubois", "Moreau", "Laurent", "Girard", "Roux", "Fournier", "Lefèvre", "Mercier"],
    cities: ["Lyon", "Bordeaux", "Lille", "Nantes", "Grenoble", "Annecy", "Bruxelles"],
  },
};
const CARRIERS = { en: ["DPD", "UPS", "DHL"], de: ["DHL", "Hermes", "DPD", "GLS"], fr: ["Colissimo", "Chronopost", "DPD", "GLS"] };
const DOMAINS = ["example.com", "example.org", "example.net", "mail.example"];
const NUMBERS = {
  en: ["two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve"],
  de: ["zwei", "drei", "vier", "fünf", "sechs", "sieben", "acht", "neun", "zehn", "elf", "zwölf"],
  fr: ["deux", "trois", "quatre", "cinq", "six", "sept", "huit", "neuf", "dix", "onze", "douze"],
};
const number = (language, n) => NUMBERS[language][n - 2] ?? String(n);
const COLORS_DE = {
  "moss green": "moosgrün",
  sand: "sand",
  rust: "rost",
  black: "schwarz",
  "ocean blue": "ozeanblau",
  ochre: "ocker",
  "forest green": "waldgrün",
  slate: "schiefergrau",
  "signal red": "signalrot",
  teal: "petrol",
  brown: "braun",
  grey: "grau",
  lime: "limette",
  navy: "marine",
  charcoal: "anthrazit",
  "heather blue": "blau meliert",
  orange: "orange",
  white: "weiß",
  "dusk purple": "dämmerlila",
  carbon: "carbon",
  oat: "hafer",
  pine: "kiefer",
};
const COLORS_FR = {
  "moss green": "vert mousse",
  sand: "sable",
  rust: "rouille",
  black: "noir",
  "ocean blue": "bleu océan",
  ochre: "ocre",
  "forest green": "vert forêt",
  slate: "ardoise",
  "signal red": "rouge vif",
  teal: "bleu canard",
  brown: "marron",
  grey: "gris",
  lime: "citron vert",
  navy: "marine",
  charcoal: "anthracite",
  "heather blue": "bleu chiné",
  orange: "orange",
  white: "blanc",
  "dusk purple": "violet crépuscule",
  carbon: "carbone",
  oat: "avoine",
  pine: "pin",
};
const colorIn = (language, color) => (language === "de" ? COLORS_DE[color] : language === "fr" ? COLORS_FR[color] : color) ?? color;

function slots(next, language, index) {
  const people = PEOPLE[language];
  const product = pick(next, PRODUCTS);
  const first = pick(next, people.first);
  const last = pick(next, people.last);
  const color = pick(next, product.colors);
  const otherColors = product.colors.filter((c) => c !== color);
  return {
    ticket: 48_120 + index,
    language,
    first,
    last,
    email: `${first
      .toLowerCase()
      .normalize("NFD")
      .replace(/[^a-z]/g, "")}.${last
      .toLowerCase()
      .normalize("NFD")
      .replace(/[^a-z]/g, "")}@${pick(next, DOMAINS)}`,
    city: pick(next, people.cities),
    product,
    color,
    wrongColor: otherColors.length ? pick(next, otherColors) : color,
    size: product.shoe ? pick(next, SHOE_SIZES) : pick(next, SIZES),
    order: `NW-${between(next, 410_000, 498_999)}`,
    carrier: pick(next, CARRIERS[language]),
    tracking: `${between(next, 10, 99)}${between(next, 100_000_000, 999_999_999)}${between(next, 10, 99)}`,
    days: between(next, 6, 12),
    stuck: between(next, 3, 8),
    need: between(next, 3, 9),
    amount: product.price + (next() < 0.4 ? pick(next, [34, 49, 89]) : 0),
    months: between(next, 4, 20),
    hours: between(next, 1, 3),
    company: pick(next, [
      "Bergwerk Outdoor GmbH",
      "Alpine Events Ltd",
      "Trailhead Expeditions SARL",
      "Gipfelglück e.V.",
      "Northline Guiding Ltd",
      "Les Sentiers Verts SAS",
    ]),
    vat: `${language === "de" ? "DE" : language === "fr" ? "FR" : "IE"}${between(next, 100_000_000, 999_999_999)}`,
    next,
  };
}

const sign = (s) => (s.next() < 0.5 ? s.first : `${s.first} ${s.last}`);

// Each case: the email (subject and body), how it is classified and the drafted answer, per language.
const CASES = [
  {
    category: "shipping_delay",
    weight: 16,
    languages: { en: 6, de: 3, fr: 2 },
    triage: (s) => ({
      priority: s.days >= 8 || s.need <= 5 ? "high" : "normal",
      sentiment: "negative",
      needs_human: false,
      summary: `Order ${s.order} (${s.product.name}) has not moved with ${s.carrier} for ${s.stuck} days; customer needs it within ${s.need} days.`,
    }),
    email: {
      en: (s) => [
        `Order ${s.order} still not here`,
        `Hi,\n\nI ordered the ${s.product.name} ${s.days} days ago (order ${s.order}) and the tracking hasn't changed for ${s.stuck} days. ${s.carrier} just says "parcel received at depot". I'm leaving for a trip in ${s.need} days and really need it before then.\n\nCould you check what's going on?\n\nThanks,\n${sign(s)}`,
      ],
      de: (s) => [
        `Bestellung ${s.order} kommt nicht an`,
        `Hallo,\n\nich habe vor ${number("de", s.days)} Tagen das ${s.product.name} bestellt (Bestellnummer ${s.order}). Laut ${s.carrier}-Sendungsverfolgung liegt das Paket seit ${s.stuck} Tagen im Paketzentrum und bewegt sich nicht. In ${number("de", s.need)} Tagen fahre ich in den Urlaub und bräuchte es bis dahin.\n\nKönnen Sie bitte nachsehen?\n\nViele Grüße\n${s.first} ${s.last}`,
      ],
      fr: (s) => [
        `Commande ${s.order} toujours pas reçue`,
        `Bonjour,\n\nJ'ai commandé le ${s.product.name} il y a ${number("fr", s.days)} jours (commande ${s.order}). Le suivi ${s.carrier} indique « colis en cours d'acheminement » depuis ${s.stuck} jours, sans aucune mise à jour. Je pars en randonnée dans ${number("fr", s.need)} jours et j'en ai besoin avant.\n\nPouvez-vous vérifier ?\n\nMerci d'avance,\n${s.first} ${s.last}`,
      ],
    },
    draft: {
      en: (s) =>
        `Hi ${s.first},\n\nthank you for letting us know, and I'm sorry your ${s.product.name} is taking so long. I checked order ${s.order}: ${s.carrier} received the parcel ${s.stuck} days ago but hasn't scanned it since, so I've opened an investigation with them today.\n\nBecause you need it within ${s.need} days, I've also prepared a replacement. If ${s.carrier} hasn't found the parcel by tomorrow afternoon, we'll send it by express at no cost, and you'll get the new tracking link by email.\n\nBest wishes,\nNorthwind Customer Care`,
      de: (s) =>
        `Hallo ${s.first},\n\nvielen Dank für Ihre Nachricht – es tut mir leid, dass Ihr ${s.product.name} so lange unterwegs ist. Ich habe die Bestellung ${s.order} geprüft: ${s.carrier} hat das Paket vor ${s.stuck} Tagen angenommen und seitdem nicht mehr gescannt. Ich habe heute einen Nachforschungsauftrag gestellt.\n\nDa Sie den Artikel in ${number("de", s.need)} Tagen brauchen, haben wir bereits einen Ersatz vorbereitet. Findet ${s.carrier} das Paket bis morgen Nachmittag nicht, schicken wir ihn kostenlos per Express – die neue Sendungsnummer erhalten Sie per E-Mail.\n\nIhr Northwind-Kundenservice`,
      fr: (s) =>
        `Bonjour ${s.first},\n\nmerci pour votre message, et toutes nos excuses pour ce retard. J'ai vérifié la commande ${s.order} : ${s.carrier} a pris en charge le colis il y a ${s.stuck} jours mais ne l'a plus scanné depuis. J'ai ouvert une enquête auprès du transporteur aujourd'hui.\n\nComme vous en avez besoin d'ici ${number("fr", s.need)} jours, nous avons préparé un remplacement : si le colis n'est pas retrouvé demain après-midi, nous vous l'envoyons gratuitement en express, avec le nouveau lien de suivi par e-mail.\n\nBien cordialement,\nLe service client Northwind`,
    },
  },
  {
    category: "damaged_item",
    weight: 10,
    languages: { en: 5, de: 3, fr: 2 },
    triage: (s) => ({
      priority: s.amount > 200 ? "high" : "normal",
      sentiment: "negative",
      needs_human: s.amount > 200,
      summary: `${s.product.name} from order ${s.order} arrived damaged in transit; customer asks for a replacement.`,
    }),
    email: {
      en: (s) => [
        `Damaged ${s.product.kind} – order ${s.order}`,
        `Hello,\n\nmy ${s.product.name} arrived today, but ${pick(s.next, ["one of the poles is snapped", "the fabric has a long tear along the side", "a buckle is broken off", "the box was crushed and the item is bent"])}. The outer box was dented on one corner, so I think it happened in transit. Photos attached.\n\nCould you send a replacement? Order ${s.order}.\n\n${sign(s)}`,
      ],
      de: (s) => [
        `Beschädigte Lieferung ${s.order}`,
        `Guten Tag,\n\nheute kam mein ${s.product.name} an, leider ${pick(s.next, ["mit einem gebrochenen Gestängeteil", "mit einem langen Riss im Stoff", "mit abgebrochener Schnalle"])}. Der Karton war an einer Ecke eingedrückt – vermutlich ist es beim Transport passiert. Fotos hängen an.\n\nKönnen Sie mir bitte Ersatz schicken? Bestellnummer ${s.order}.\n\nMit freundlichen Grüßen\n${s.first} ${s.last}`,
      ],
      fr: (s) => [
        `Article abîmé – commande ${s.order}`,
        `Bonjour,\n\nj'ai reçu aujourd'hui mon ${s.product.name}, mais ${pick(s.next, ["un arceau est cassé", "le tissu est déchiré sur le côté", "une boucle est cassée"])}. Le carton était enfoncé dans un coin, cela s'est sans doute produit pendant le transport. Je joins des photos.\n\nPourriez-vous m'envoyer un remplacement ? Commande ${s.order}.\n\nCordialement,\n${s.first} ${s.last}`,
      ],
    },
    draft: {
      en: (s) =>
        `Hi ${s.first},\n\nI'm sorry your ${s.product.name} arrived damaged – thank you for the photos, they make this easy. I've sent a replacement in ${colorIn("en", s.color)} today; you'll get the tracking link by email within a few hours.\n\n${s.amount > 200 ? "Because of the item's value, we'll email you a prepaid label for the damaged one – please send it back within 30 days." : "There's no need to send the damaged one back; please recycle it or keep the parts."}\n\nBest wishes,\nNorthwind Customer Care`,
      de: (s) =>
        `Hallo ${s.first},\n\nes tut mir leid, dass Ihr ${s.product.name} beschädigt angekommen ist – danke für die Fotos. Ich habe heute einen Ersatz in ${colorIn("de", s.color)} verschickt; die Sendungsnummer erhalten Sie in den nächsten Stunden per E-Mail.\n\n${s.amount > 200 ? "Wegen des Warenwerts schicken wir Ihnen ein kostenloses Rücksendeetikett für das beschädigte Exemplar – bitte senden Sie es innerhalb von 30 Tagen zurück." : "Das beschädigte Exemplar müssen Sie nicht zurückschicken."}\n\nIhr Northwind-Kundenservice`,
      fr: (s) =>
        `Bonjour ${s.first},\n\nnous sommes désolés que votre ${s.product.name} soit arrivé abîmé, et merci pour les photos. Un remplacement coloris ${colorIn("fr", s.color)} est parti aujourd'hui ; vous recevrez le lien de suivi par e-mail dans quelques heures.\n\n${s.amount > 200 ? "Vu la valeur de l'article, nous vous envoyons une étiquette de retour prépayée pour l'exemplaire abîmé, à renvoyer sous 30 jours." : "Inutile de nous renvoyer l'article abîmé."}\n\nBien cordialement,\nLe service client Northwind`,
    },
  },
  {
    category: "wrong_item",
    weight: 6,
    languages: { en: 4, de: 2, fr: 1 },
    triage: (s) => ({
      priority: "normal",
      sentiment: "negative",
      needs_human: false,
      summary: `Customer received ${s.product.name} in ${s.wrongColor} instead of ${s.color} (order ${s.order}); wants an exchange.`,
    }),
    email: {
      en: (s) => [
        `Wrong colour sent`,
        `Hi, I ordered the ${s.product.name} in ${s.color} but got ${s.wrongColor}. Order ${s.order}. I'd really prefer the colour I ordered – can you swap it? Happy to send this one back.\n\nCheers, ${s.first}`,
      ],
      de: (s) => [
        `Falsche Farbe geliefert (${s.order})`,
        `Hallo, ich habe das ${s.product.name} in ${colorIn("de", s.color)} bestellt, bekommen habe ich aber ${colorIn("de", s.wrongColor)}. Können Sie das umtauschen? Ich schicke das falsche gern zurück.\n\nDanke und Gruß\n${s.first}`,
      ],
      fr: (s) => [
        `Mauvaise couleur reçue`,
        `Bonjour, j'ai commandé le ${s.product.name} en ${colorIn("fr", s.color)} et j'ai reçu du ${colorIn("fr", s.wrongColor)} (commande ${s.order}). Est-il possible de l'échanger ? Je peux renvoyer celui-ci.\n\nMerci, ${s.first}`,
      ],
    },
    draft: {
      en: (s) =>
        `Hi ${s.first},\n\nsorry about the mix-up! I've sent the ${s.product.name} in ${s.color} today, and you'll find a prepaid return label for the ${s.wrongColor} one in your account under order ${s.order}. Please drop it off within 30 days – no need to wait for the new one first.\n\nBest wishes,\nNorthwind Customer Care`,
      de: (s) =>
        `Hallo ${s.first},\n\nentschuldigen Sie die Verwechslung! Das ${s.product.name} in ${colorIn("de", s.color)} ist heute an Sie rausgegangen. Ein kostenloses Rücksendeetikett für das ${colorIn("de", s.wrongColor)}e Exemplar finden Sie in Ihrem Konto unter der Bestellung ${s.order} – bitte innerhalb von 30 Tagen abgeben.\n\nIhr Northwind-Kundenservice`,
      fr: (s) =>
        `Bonjour ${s.first},\n\ntoutes nos excuses pour l'erreur ! Le ${s.product.name} coloris ${colorIn("fr", s.color)} vous a été expédié aujourd'hui. L'étiquette de retour prépayée pour l'autre coloris est disponible dans votre compte, commande ${s.order} ; vous avez 30 jours pour le déposer.\n\nBien cordialement,\nLe service client Northwind`,
    },
  },
  {
    category: "return_request",
    weight: 14,
    languages: { en: 6, de: 4, fr: 2 },
    triage: (s) => ({
      priority: "normal",
      sentiment: "neutral",
      needs_human: false,
      summary: `Customer wants to return the ${s.product.name}${s.product.sized ? ` (size ${s.size})` : ""} from order ${s.order}; unused.`,
    }),
    email: {
      en: (s) => [
        `Return – ${s.product.name}`,
        `Hello,\n\nI'd like to return the ${s.product.name}${s.product.sized ? ` (size ${s.size})` : ""} from order ${s.order}. ${s.product.sized ? `It fits too ${pick(s.next, ["small", "large", "tight across the shoulders"])}.` : "It's not quite what I was looking for."} It's unused, tags still on. How do I get a return label?\n\nKind regards,\n${s.first} ${s.last}`,
      ],
      de: (s) => [
        `Rücksendung ${s.order}`,
        `Hallo,\n\nich möchte das ${s.product.name}${s.product.sized ? ` in Größe ${s.size}` : ""} aus der Bestellung ${s.order} zurückschicken${s.product.sized ? `, es ist mir leider zu ${pick(s.next, ["klein", "groß", "eng"])}` : ""}. Es ist ungetragen und mit Etikett. Wie bekomme ich ein Rücksendeetikett?\n\nBeste Grüße\n${s.first} ${s.last}`,
      ],
      fr: (s) => [
        `Retour d'un article`,
        `Bonjour,\n\nje souhaite retourner le ${s.product.name}${s.product.sized ? ` (taille ${s.size})` : ""} de la commande ${s.order}${s.product.sized ? `, il est trop ${pick(s.next, ["petit", "grand"])}` : ""}. Il n'a pas été utilisé et porte encore l'étiquette. Comment obtenir une étiquette de retour ?\n\nCordialement,\n${s.first} ${s.last}`,
      ],
    },
    draft: {
      en: (s) =>
        `Hi ${s.first},\n\nof course! You can print a free return label for order ${s.order} in your account under *Orders → Return an item*. Once the ${s.product.name} reaches our warehouse, we refund €${s.amount} to your original payment method within 5 business days.\n\n${s.product.sized ? `If you'd like a different size instead, just reply with it and we'll send it out straight away.\n\n` : ""}Best wishes,\nNorthwind Customer Care`,
      de: (s) =>
        `Hallo ${s.first},\n\nsehr gern! Ein kostenloses Rücksendeetikett für die Bestellung ${s.order} können Sie in Ihrem Konto unter *Bestellungen → Artikel zurücksenden* ausdrucken. Sobald das ${s.product.name} bei uns im Lager ist, erstatten wir ${s.amount} € innerhalb von 5 Werktagen.\n\n${s.product.sized ? "Möchten Sie lieber eine andere Größe? Antworten Sie einfach mit der Größe, dann schicken wir sie sofort los.\n\n" : ""}Ihr Northwind-Kundenservice`,
      fr: (s) =>
        `Bonjour ${s.first},\n\nbien sûr ! Vous pouvez imprimer une étiquette de retour gratuite pour la commande ${s.order} depuis votre compte, rubrique *Commandes → Retourner un article*. Dès réception du ${s.product.name} à l'entrepôt, nous vous remboursons ${s.amount} € sous 5 jours ouvrés.\n\nBien cordialement,\nLe service client Northwind`,
    },
  },
  {
    category: "refund_status",
    weight: 9,
    languages: { en: 4, de: 3, fr: 2 },
    triage: (s) => ({
      priority: s.days >= 9 ? "high" : "normal",
      sentiment: "negative",
      needs_human: s.amount > 150,
      summary: `Return for order ${s.order} reached the warehouse ${s.days} days ago; refund of €${s.amount} not received yet.`,
    }),
    email: {
      en: (s) => [
        `Still waiting for my refund`,
        `Hi there,\n\nI sent back the ${s.product.name} with your prepaid label and ${s.carrier} shows it was delivered to your warehouse ${s.days} days ago. I still haven't received my refund of €${s.amount} (order ${s.order}).\n\nWhen can I expect it?\n\n${sign(s)}`,
      ],
      de: (s) => [
        `Erstattung fehlt noch – ${s.order}`,
        `Guten Tag,\n\nmeine Rücksendung (${s.product.name}, Bestellung ${s.order}) ist laut ${s.carrier} seit ${s.days} Tagen bei Ihnen. Die Erstattung über ${s.amount} € ist aber noch nicht auf meinem Konto. Wann kann ich damit rechnen?\n\nFreundliche Grüße\n${s.first} ${s.last}`,
      ],
      fr: (s) => [
        `Remboursement non reçu`,
        `Bonjour,\n\nmon retour (${s.product.name}, commande ${s.order}) a été livré à votre entrepôt il y a ${number("fr", s.days)} jours d'après ${s.carrier}, mais je n'ai toujours pas reçu le remboursement de ${s.amount} €. Pouvez-vous me dire quand il arrivera ?\n\n${s.first} ${s.last}`,
      ],
    },
    draft: {
      en: (s) =>
        `Hi ${s.first},\n\nthanks for your patience, and sorry for the wait. Your return for order ${s.order} was checked in at our warehouse, but the refund was held for a manual check. I've released it just now: €${s.amount} is on its way to your original payment method and usually shows up within 3 to 5 business days, depending on your bank.\n\nBest wishes,\nNorthwind Customer Care`,
      de: (s) =>
        `Hallo ${s.first},\n\ndanke für Ihre Geduld, und entschuldigen Sie die Wartezeit. Ihre Rücksendung zur Bestellung ${s.order} ist bei uns eingegangen, die Erstattung hing aber in einer manuellen Prüfung. Ich habe sie eben freigegeben: ${s.amount} € gehen auf Ihr ursprüngliches Zahlungsmittel und sind je nach Bank in 3 bis 5 Werktagen sichtbar.\n\nIhr Northwind-Kundenservice`,
      fr: (s) =>
        `Bonjour ${s.first},\n\nmerci pour votre patience. Votre retour (commande ${s.order}) a bien été réceptionné, mais le remboursement était bloqué pour une vérification manuelle. Je viens de le débloquer : ${s.amount} € seront recrédités sur votre moyen de paiement d'origine sous 3 à 5 jours ouvrés selon votre banque.\n\nBien cordialement,\nLe service client Northwind`,
    },
  },
  {
    category: "payment_issue",
    weight: 4,
    languages: { en: 3, de: 2, fr: 1 },
    triage: (s) => ({
      priority: "high",
      sentiment: "negative",
      needs_human: true,
      summary: `Customer was charged twice (2 × €${s.amount}) for order ${s.order}; asks for the duplicate to be refunded.`,
    }),
    email: {
      en: (s) => [
        `Charged twice for ${s.order}`,
        `Hello,\n\nmy card was charged twice for order ${s.order} – two payments of €${s.amount} on the same day. Screenshot of my bank statement attached. Please refund the duplicate.\n\n${s.first} ${s.last}`,
      ],
      de: (s) => [
        `Doppelte Abbuchung`,
        `Hallo,\n\nfür die Bestellung ${s.order} wurde meine Karte zweimal mit je ${s.amount} € belastet (Screenshot anbei). Bitte erstatten Sie die doppelte Zahlung.\n\n${s.first} ${s.last}`,
      ],
      fr: (s) => [
        `Double prélèvement`,
        `Bonjour,\n\nma carte a été débitée deux fois de ${s.amount} € pour la commande ${s.order}. Vous trouverez une capture de mon relevé en pièce jointe. Merci de rembourser le paiement en double.\n\n${s.first} ${s.last}`,
      ],
    },
    draft: {
      en: (s) =>
        `Hi ${s.first},\n\nthank you for the screenshot and sorry for the trouble. I can see both payments for order ${s.order}; the second one is a duplicate from a payment retry. I've asked our payments team to refund €${s.amount} today – you'll get a confirmation email once it's done, and your bank should show it within 5 business days.\n\nBest wishes,\nNorthwind Customer Care`,
      de: (s) =>
        `Hallo ${s.first},\n\ndanke für den Screenshot, und entschuldigen Sie den Ärger. Ich sehe beide Zahlungen zur Bestellung ${s.order}; die zweite stammt aus einem automatischen Zahlungsversuch. Unser Zahlungsteam erstattet ${s.amount} € noch heute – Sie erhalten eine Bestätigung per E-Mail, auf dem Konto ist der Betrag in spätestens 5 Werktagen.\n\nIhr Northwind-Kundenservice`,
      fr: (s) =>
        `Bonjour ${s.first},\n\nmerci pour la capture et désolés pour ce désagrément. Je vois bien les deux paiements pour la commande ${s.order} ; le second provient d'une nouvelle tentative automatique. Notre équipe paiements rembourse ${s.amount} € aujourd'hui, vous recevrez une confirmation par e-mail.\n\nBien cordialement,\nLe service client Northwind`,
    },
  },
  {
    category: "account_access",
    weight: 5,
    languages: { en: 3, de: 2, fr: 1 },
    triage: () => ({
      priority: "normal",
      sentiment: "neutral",
      needs_human: false,
      summary: "Customer cannot sign in; password reset email does not arrive.",
    }),
    email: {
      en: (s) => [
        `Can't log in`,
        `Hi, I can't log in to my account anymore. The password reset email never arrives (checked spam too). My email is ${s.email}. Can you help?\n\n${s.first}`,
      ],
      de: (s) => [
        `Login funktioniert nicht`,
        `Hallo, ich komme nicht mehr in mein Kundenkonto. Die E-Mail zum Zurücksetzen des Passworts kommt nicht an (auch nicht im Spam). Meine Adresse: ${s.email}.\n\nGruß ${s.first}`,
      ],
      fr: (s) => [
        `Impossible de me connecter`,
        `Bonjour, je n'arrive plus à me connecter à mon compte et l'e-mail de réinitialisation du mot de passe n'arrive pas (j'ai vérifié les spams). Mon adresse : ${s.email}.\n\nMerci, ${s.first}`,
      ],
    },
    draft: {
      en: (s) =>
        `Hi ${s.first},\n\nsorry about that! Your address had bounced an earlier email, so our system stopped sending to it. I've cleared that flag and sent a fresh reset link to ${s.email} just now – it's valid for one hour. If it still doesn't arrive, reply here and I'll set up the account with you directly.\n\nBest wishes,\nNorthwind Customer Care`,
      de: (s) =>
        `Hallo ${s.first},\n\nentschuldigen Sie! An Ihre Adresse war eine frühere E-Mail nicht zustellbar, deshalb hat unser System keine weiteren geschickt. Ich habe das zurückgesetzt und Ihnen eben einen neuen Link an ${s.email} geschickt; er ist eine Stunde gültig.\n\nIhr Northwind-Kundenservice`,
      fr: (s) =>
        `Bonjour ${s.first},\n\ndésolés ! Un ancien e-mail était revenu en erreur, notre système avait donc cessé d'écrire à cette adresse. C'est corrigé : un nouveau lien de réinitialisation vient de partir vers ${s.email}, valable une heure.\n\nBien cordialement,\nLe service client Northwind`,
    },
  },
  {
    category: "product_question",
    weight: 12,
    languages: { en: 6, de: 3, fr: 2 },
    triage: (s) => ({
      priority: "low",
      sentiment: "positive",
      needs_human: false,
      summary: `Pre-sale question whether the ${s.product.name} suits ${s.use}.`,
    }),
    prepare: (s) => {
      s.use = pick(s.next, [
        "winter camping above the tree line",
        "a two-week trek in Norway",
        "everyday commuting by bike",
        "multi-day hikes with a lot of rain",
        "a first Kilimanjaro climb",
      ]);
      s.useDe = {
        "winter camping above the tree line": "Wintercamping oberhalb der Baumgrenze",
        "a two-week trek in Norway": "eine zweiwöchige Tour in Norwegen",
        "everyday commuting by bike": "das tägliche Pendeln mit dem Rad",
        "multi-day hikes with a lot of rain": "mehrtägige Touren bei viel Regen",
        "a first Kilimanjaro climb": "eine erste Kilimandscharo-Besteigung",
      }[s.use];
      s.useFr = {
        "winter camping above the tree line": "du camping d'hiver en altitude",
        "a two-week trek in Norway": "un trek de deux semaines en Norvège",
        "everyday commuting by bike": "les trajets quotidiens à vélo",
        "multi-day hikes with a lot of rain": "des randonnées de plusieurs jours sous la pluie",
        "a first Kilimanjaro climb": "une première ascension du Kilimandjaro",
      }[s.use];
      s.verdict = s.product.kind === "tent" && s.use.includes("winter") ? "three-season" : "fits";
    },
    email: {
      en: (s) => [
        `Question about the ${s.product.name}`,
        `Hi! Is the ${s.product.name} a good choice for ${s.use}? I'm deciding between it and a few others. ${s.product.sized ? `I'm usually a ${s.size} – does it run true to size?` : "How much does it weigh?"}\n\nThanks!\n${s.first}`,
      ],
      de: (s) => [
        `Frage zum ${s.product.name}`,
        `Hallo! Ist das ${s.product.name} für ${s.useDe} geeignet? ${s.product.sized ? `Ich trage normalerweise ${s.size} – fällt es normal aus?` : "Wie schwer ist es?"}\n\nDanke und viele Grüße\n${s.first}`,
      ],
      fr: (s) => [
        `Question sur le ${s.product.name}`,
        `Bonjour ! Le ${s.product.name} convient-il pour ${s.useFr} ? ${s.product.sized ? `Je fais habituellement du ${s.size}, la taille est-elle standard ?` : "Quel est son poids ?"}\n\nMerci, ${s.first}`,
      ],
    },
    draft: {
      en: (s) =>
        `Hi ${s.first},\n\ngreat question! ${s.verdict === "three-season" ? `The ${s.product.name} is a three-season tent, so for ${s.use} we'd point you to a four-season model with snow flaps instead.` : `The ${s.product.name} is a solid choice for ${s.use} – it's one of our most-reviewed items for exactly that.`} ${s.product.sized ? `It runs true to size, so ${s.size} should fit; if you're between sizes, size up for layering.` : "You'll find the packed weight and dimensions under *Specifications* on the product page."}\n\nAnd if it doesn't work out, returns are free for 30 days.\n\nHappy trails,\nNorthwind Customer Care`,
      de: (s) =>
        `Hallo ${s.first},\n\ngute Frage! ${s.verdict === "three-season" ? `Das ${s.product.name} ist ein Drei-Jahreszeiten-Zelt; für ${s.useDe} empfehlen wir ein Vier-Jahreszeiten-Modell mit Schneelappen.` : `Das ${s.product.name} passt sehr gut für ${s.useDe} – genau dafür wird es bei uns am häufigsten gekauft.`} ${s.product.sized ? `Es fällt normal aus, ${s.size} sollte also passen.` : "Gewicht und Packmaß finden Sie auf der Produktseite unter *Technische Daten*."}\n\nUnd falls es doch nicht passt: Die Rücksendung ist 30 Tage kostenlos.\n\nIhr Northwind-Kundenservice`,
      fr: (s) =>
        `Bonjour ${s.first},\n\nbonne question ! ${s.verdict === "three-season" ? `Le ${s.product.name} est une tente trois saisons ; pour ${s.useFr}, nous conseillons plutôt un modèle quatre saisons avec bavettes à neige.` : `Le ${s.product.name} convient très bien pour ${s.useFr}.`} ${s.product.sized ? `Il taille normalement, le ${s.size} devrait donc vous aller.` : "Le poids et l'encombrement figurent dans l'onglet *Caractéristiques* de la fiche produit."}\n\nLes retours sont gratuits pendant 30 jours.\n\nBien cordialement,\nLe service client Northwind`,
    },
  },
  {
    category: "cancellation",
    weight: 5,
    languages: { en: 3, de: 2, fr: 1 },
    triage: (s) => ({
      priority: "urgent",
      sentiment: "neutral",
      needs_human: false,
      summary: `Customer asks to cancel order ${s.order}, placed ${s.hours} hours ago, before it ships.`,
    }),
    email: {
      en: (s) => [
        `Please cancel ${s.order}`,
        `Hi, please cancel my order ${s.order} – I ordered the wrong ${s.product.sized ? "size" : "item"} by mistake. I placed it about ${s.hours === 1 ? "an hour" : `${s.hours} hours`} ago so hopefully it hasn't shipped yet.\n\nThank you!\n${s.first} ${s.last}`,
      ],
      de: (s) => [
        `Stornierung ${s.order}`,
        `Hallo, bitte stornieren Sie meine Bestellung ${s.order}, ich habe mich in der ${s.product.sized ? "Größe" : "Variante"} vertan. Sie ist erst ${s.hours === 1 ? "eine Stunde" : `${number("de", s.hours)} Stunden`} alt.\n\nDanke!\n${s.first} ${s.last}`,
      ],
      fr: (s) => [
        `Annulation de commande`,
        `Bonjour, merci d'annuler ma commande ${s.order}, je me suis trompé de ${s.product.sized ? "taille" : "modèle"}. Elle a été passée il y a ${s.hours === 1 ? "une heure" : `${number("fr", s.hours)} heures`}.\n\nMerci, ${s.first}`,
      ],
    },
    draft: {
      en: (s) =>
        `Hi ${s.first},\n\ndone – order ${s.order} hadn't been packed yet, so I've cancelled it. The payment of €${s.amount} was only authorised, not taken, and the hold on your card will disappear within a few days.\n\nIf you'd like to order the right ${s.product.sized ? "size" : "item"}, it's still in stock.\n\nBest wishes,\nNorthwind Customer Care`,
      de: (s) =>
        `Hallo ${s.first},\n\nerledigt – die Bestellung ${s.order} war noch nicht verpackt, ich habe sie storniert. Die ${s.amount} € waren nur reserviert und nicht abgebucht; die Reservierung verschwindet in wenigen Tagen.\n\nIhr Northwind-Kundenservice`,
      fr: (s) =>
        `Bonjour ${s.first},\n\nc'est fait : la commande ${s.order} n'était pas encore préparée, je l'ai annulée. Les ${s.amount} € étaient seulement autorisés et non débités ; l'autorisation disparaîtra sous quelques jours.\n\nBien cordialement,\nLe service client Northwind`,
    },
  },
  {
    category: "warranty_claim",
    weight: 6,
    languages: { en: 3, de: 3, fr: 1 },
    triage: (s) => ({
      priority: "normal",
      sentiment: "neutral",
      needs_human: s.months > 18,
      summary: `Zip on the ${s.product.name} failed after ${s.months} months of use (order ${s.order}); customer asks about warranty.`,
    }),
    email: {
      en: (s) => [
        `Warranty – broken zip`,
        `Hello,\n\nthe main zip on my ${s.product.name} broke after about ${s.months} months of normal use. I bought it from you (order ${s.order}). Is this covered by the warranty?\n\nRegards,\n${s.first} ${s.last}`,
      ],
      de: (s) => [
        `Garantiefall Reißverschluss`,
        `Guten Tag,\n\nbei meinem ${s.product.name} ist nach etwa ${s.months} Monaten normaler Nutzung der Hauptreißverschluss kaputtgegangen (Bestellung ${s.order}). Ist das ein Garantiefall?\n\nMit freundlichen Grüßen\n${s.first} ${s.last}`,
      ],
      fr: (s) => [
        `Garantie – fermeture éclair cassée`,
        `Bonjour,\n\nla fermeture éclair principale de mon ${s.product.name} a cassé après environ ${s.months} mois d'utilisation normale (commande ${s.order}). Est-ce couvert par la garantie ?\n\n${s.first} ${s.last}`,
      ],
    },
    draft: {
      en: (s) =>
        `Hi ${s.first},\n\nyes – zips are covered by our warranty, and your ${s.product.name} is well within it. You'll find a free warranty label for order ${s.order} in your account. Send the item to our repair partner and we'll replace the zip, usually within 10 days; if it can't be repaired, we'll send you a new one.\n\nBest wishes,\nNorthwind Customer Care`,
      de: (s) =>
        `Hallo ${s.first},\n\nja – Reißverschlüsse sind durch unsere Garantie abgedeckt. Ein kostenloses Garantie-Etikett zur Bestellung ${s.order} finden Sie in Ihrem Konto. Unser Reparaturpartner tauscht den Reißverschluss meist innerhalb von 10 Tagen; ist eine Reparatur nicht möglich, bekommen Sie ein neues Produkt.\n\nIhr Northwind-Kundenservice`,
      fr: (s) =>
        `Bonjour ${s.first},\n\noui, les fermetures éclair sont couvertes par notre garantie. Une étiquette de retour garantie gratuite pour la commande ${s.order} vous attend dans votre compte. Notre partenaire remplace la fermeture sous 10 jours environ ; si la réparation est impossible, nous vous envoyons un article neuf.\n\nBien cordialement,\nLe service client Northwind`,
    },
  },
  {
    category: "invoice_request",
    weight: 4,
    languages: { en: 2, de: 3, fr: 1 },
    triage: (s) => ({
      priority: "low",
      sentiment: "neutral",
      needs_human: false,
      summary: `Business customer requests an invoice with VAT number for order ${s.order}.`,
    }),
    email: {
      en: (s) => [
        `Invoice with VAT number`,
        `Hi, could you send me an invoice for order ${s.order} with our company details? ${s.company}, VAT ${s.vat}. Thanks! ${s.first}`,
      ],
      de: (s) => [
        `Rechnung mit USt-IdNr.`,
        `Hallo, bitte stellen Sie mir für die Bestellung ${s.order} eine Rechnung auf ${s.company}, USt-IdNr. ${s.vat}, aus. Vielen Dank!\n${s.first} ${s.last}`,
      ],
      fr: (s) => [
        `Facture avec numéro de TVA`,
        `Bonjour, pourriez-vous m'envoyer une facture pour la commande ${s.order} au nom de ${s.company}, TVA ${s.vat} ? Merci, ${s.first}`,
      ],
    },
    draft: {
      en: (s) =>
        `Hi ${s.first},\n\nhere you go – the invoice for order ${s.order}, issued to ${s.company} with VAT number ${s.vat}, is attached. For future orders you can add the VAT number under *Account → Billing*, and every invoice will include it automatically.\n\nBest wishes,\nNorthwind Customer Care`,
      de: (s) =>
        `Hallo ${s.first},\n\nanbei die Rechnung zur Bestellung ${s.order} auf ${s.company} mit der USt-IdNr. ${s.vat}. Für künftige Bestellungen können Sie die Nummer unter *Konto → Rechnungsdaten* hinterlegen, dann steht sie automatisch auf jeder Rechnung.\n\nIhr Northwind-Kundenservice`,
      fr: (s) =>
        `Bonjour ${s.first},\n\nvous trouverez ci-joint la facture de la commande ${s.order} au nom de ${s.company}, TVA ${s.vat}. Vous pouvez enregistrer ce numéro dans *Compte → Facturation* pour qu'il figure sur vos prochaines factures.\n\nBien cordialement,\nLe service client Northwind`,
    },
  },
];

function weighted(next, entries) {
  const total = entries.reduce((sum, [, weight]) => sum + weight, 0);
  let roll = next() * total;
  for (const [value, weight] of entries) if ((roll -= weight) < 0) return value;
  return entries.at(-1)[0];
}

// One support ticket: the email, its classification and the drafted answer.
export function ticket(index) {
  const next = random(0x5eed_0000 + index * 7919);
  const kind = weighted(
    next,
    CASES.map((c) => [c, c.weight]),
  );
  const language = weighted(next, Object.entries(kind.languages));
  const s = slots(next, language, index);
  kind.prepare?.(s);
  const [subject, body] = kind.email[language](s);
  const triage = kind.triage(s);
  const classification = {
    category: kind.category,
    priority: triage.priority,
    language,
    sentiment: triage.sentiment,
    order_id: kind.category === "account_access" || kind.category === "product_question" ? null : s.order,
    needs_human: triage.needs_human,
    summary: triage.summary,
  };
  const email = `Ticket #${s.ticket} · via email\nFrom: ${s.first} ${s.last} <${s.email}>\nSubject: ${subject}\n\n${body}`;
  const context =
    classification.order_id === null
      ? "No order found for this customer."
      : `Order ${s.order}: ${s.product.name}${s.product.sized ? `, size ${s.size}` : ""}, ${s.color}, €${s.amount}. Shipped with ${s.carrier} (tracking ${s.tracking}) to ${s.city}.`;
  return {
    index,
    language,
    category: kind.category,
    draftable: next() < 0.62,
    classify: { user: email, reply: JSON.stringify(classification, null, 2) },
    draft: {
      user: `Draft a reply to this ticket.\n\nCategory: ${kind.category} · Priority: ${triage.priority} · Language: ${language}\n${context}\n\n---\n${email}`,
      reply: kind.draft[language](s),
    },
  };
}

const THREADS = [
  {
    topic: "the checkout latency spike",
    lines: (n) => [
      `${n[0]}: p95 on /checkout jumped to 4.2s around 13:10, anyone deploying?`,
      `${n[1]}: not us, last deploy was this morning`,
      `${n[2]}: Kaeso shows Anthropic returning 529s, the sturdy alias is falling back to gpt-6-sol`,
      `${n[0]}: that explains the slower replies, fallback adds ~1.5s`,
      `${n[1]}: should we switch the default to fast until it settles?`,
      `${n[2]}: no, errors stayed at zero thanks to the fallback. let's just watch it`,
      `${n[0]}: ok, back to normal since 13:55 👍`,
    ],
    summary:
      "*Checkout latency spike (13:10–13:55)*\n• Cause: Anthropic was overloaded (529s); Kaeso's `sturdy` alias fell back to gpt-6-sol, adding ~1.5 s per reply.\n• Impact: slower replies, *no failed requests*.\n• Decision: no change to the alias; latency back to normal at 13:55.\n• Follow-up: none needed.",
  },
  {
    topic: "the autumn sale planning",
    lines: (n) => [
      `${n[0]}: sale starts Thursday 18:00, email goes to 240k subscribers`,
      `${n[1]}: last time checkout traffic tripled in the first hour`,
      `${n[2]}: our Kaeso project limit is 60 req/min, we'll hit it`,
      `${n[1]}: raise it for the evening or let the chatbot queue?`,
      `${n[0]}: raise it to 120 for Thursday evening, I'll put it back Friday`,
      `${n[2]}: also pre-warm the product embeddings on Wednesday night`,
    ],
    summary:
      "*Autumn sale prep*\n• Sale starts *Thursday 18:00*, email to ~240k subscribers; expect ~3× checkout traffic in the first hour.\n• Raise the Kaeso request-rate limit on Checkout assistant from 60 to *120/min* for Thursday evening, back on Friday.\n• Re-run the product embedding job on Wednesday night.",
  },
  {
    topic: "the returns label bug",
    lines: (n) => [
      `${n[0]}: customers in Austria get a German return label, DHL rejects it`,
      `${n[1]}: since when?`,
      `${n[0]}: first ticket three days ago, 14 so far`,
      `${n[2]}: carrier mapping uses the billing country instead of shipping country, fix is a one-liner`,
      `${n[1]}: ship it today please, and let's mail the 14 customers a new label`,
    ],
    summary:
      "*Return labels for Austria*\n• Austrian customers get German DHL labels, which DHL rejects (14 tickets in 3 days).\n• Cause: carrier mapping uses the billing instead of the shipping country.\n• Fix ships today; Customer Care sends the 14 affected customers new labels.",
  },
  {
    topic: "moving support triage to Haiku 5.5",
    lines: (n) => [
      `${n[0]}: triage evals on haiku 5.5: 96.8% category accuracy vs 95.1% on 4.5`,
      `${n[1]}: and cost?`,
      `${n[0]}: about a tenth per ticket in Kaeso's usage view`,
      `${n[2]}: latency is lower too. let's switch Monday and keep 4.5 as a fallback for a week`,
      `${n[1]}: 👍 I'll update the alias`,
    ],
    summary:
      "*Support triage → Claude Haiku 5.5*\n• Evals: 96.8% category accuracy (vs 95.1% on Haiku 4.5), lower latency, about a tenth of the cost per ticket.\n• Switch on Monday; keep Haiku 4.5 as fallback for one week.\n• Owner: alias update.",
  },
  {
    topic: "the warehouse scanner outage",
    lines: (n) => [
      `${n[0]}: scanners in hall B are down, nothing can be packed`,
      `${n[1]}: vendor says firmware update failed, rolling back now`,
      `${n[2]}: ~1,800 orders waiting, cutoff for same-day is 15:00`,
      `${n[1]}: back up at 12:40`,
      `${n[2]}: we'll make the cutoff, maybe 100 orders slip to tomorrow`,
    ],
    summary:
      "*Hall B scanner outage*\n• A failed firmware update took the scanners down; the vendor rolled it back, working again at 12:40.\n• ~1,800 orders were waiting; about 100 may miss today's 15:00 same-day cutoff.\n• Customer Care should expect a few delay tickets tomorrow.",
  },
];
const NAMES = ["Lin", "Noor", "Mateo", "Priya", "Jonas", "Ada"];

const TRANSLATIONS = [
  [
    "Your order is on its way! Track it any time from your account.",
    "Ihre Bestellung ist unterwegs! Sie können sie jederzeit in Ihrem Konto verfolgen.",
    "Votre commande est en route ! Suivez-la à tout moment depuis votre compte.",
  ],
  [
    "Free returns for 30 days – no questions asked.",
    "30 Tage kostenlose Rücksendung – ohne Wenn und Aber.",
    "Retours gratuits pendant 30 jours, sans justification.",
  ],
  [
    "Our warehouse is closed on public holidays; orders ship the next working day.",
    "An Feiertagen bleibt unser Lager geschlossen; Bestellungen gehen am nächsten Werktag raus.",
    "Notre entrepôt est fermé les jours fériés ; les commandes partent le jour ouvré suivant.",
  ],
  [
    "Tents and backpacks come with a five-year warranty.",
    "Auf Zelte und Rucksäcke gibt es fünf Jahre Garantie.",
    "Les tentes et sacs à dos sont garantis cinq ans.",
  ],
  [
    "Sign up for restock alerts and we'll email you the moment it's back.",
    "Melden Sie sich für die Verfügbarkeitsbenachrichtigung an – wir schreiben Ihnen, sobald der Artikel wieder da ist.",
    "Inscrivez-vous aux alertes de réassort : nous vous écrivons dès son retour en stock.",
  ],
  [
    "Orders placed before 3 pm ship the same day.",
    "Bestellungen bis 15 Uhr verlassen noch am selben Tag unser Lager.",
    "Les commandes passées avant 15 h sont expédiées le jour même.",
  ],
  [
    "We're sorry – this item is no longer available in your size.",
    "Leider ist dieser Artikel in Ihrer Größe nicht mehr verfügbar.",
    "Désolés, cet article n'est plus disponible dans votre taille.",
  ],
  [
    "Repair, don't replace: send us your worn gear and we'll fix it at cost.",
    "Reparieren statt ersetzen: Schicken Sie uns Ihre Ausrüstung, wir reparieren sie zum Selbstkostenpreis.",
    "Réparer plutôt que remplacer : envoyez-nous votre équipement usé, nous le réparons à prix coûtant.",
  ],
];

const METRICS = [
  [
    "return rate",
    "returns per product category",
    "SELECT p.category,\n       count(r.id)::float / nullif(count(DISTINCT o.id), 0) AS return_rate\nFROM orders o\nJOIN order_items i ON i.order_id = o.id\nJOIN products p ON p.id = i.product_id\nLEFT JOIN returns r ON r.order_item_id = i.id\nWHERE o.created_at >= now() - interval '{days} days'\nGROUP BY p.category\nORDER BY return_rate DESC;",
  ],
  [
    "average order value",
    "AOV by country",
    "SELECT shipping_country,\n       round(avg(total_eur), 2) AS aov,\n       count(*) AS orders\nFROM orders\nWHERE created_at >= now() - interval '{days} days'\n  AND status <> 'cancelled'\nGROUP BY shipping_country\nORDER BY orders DESC;",
  ],
  [
    "late deliveries",
    "late parcels per carrier",
    "SELECT carrier,\n       count(*) FILTER (WHERE delivered_at > promised_at) AS late,\n       count(*) AS shipped,\n       round(100.0 * count(*) FILTER (WHERE delivered_at > promised_at) / count(*), 1) AS late_pct\nFROM shipments\nWHERE shipped_at >= now() - interval '{days} days'\nGROUP BY carrier\nORDER BY late_pct DESC;",
  ],
  [
    "ticket volume",
    "support tickets per category and day",
    "SELECT date_trunc('day', created_at) AS day,\n       category,\n       count(*) AS tickets\nFROM support_tickets\nWHERE created_at >= now() - interval '{days} days'\nGROUP BY 1, 2\nORDER BY 1, 3 DESC;",
  ],
  [
    "chatbot conversion",
    "checkout assistant sessions that ended in an order",
    "SELECT date_trunc('week', s.started_at) AS week,\n       count(*) AS sessions,\n       count(o.id) AS orders,\n       round(100.0 * count(o.id) / count(*), 1) AS conversion_pct\nFROM assistant_sessions s\nLEFT JOIN orders o ON o.session_id = s.id\nWHERE s.started_at >= now() - interval '{days} days'\nGROUP BY 1\nORDER BY 1;",
  ],
];

const POLICY = [
  [
    "what's our return window for sale items?",
    "Same as everything else: *30 days from delivery*, free with the prepaid label. Only exception: personalised items (engraved bottles) can't be returned.",
  ],
  [
    "can customers swap sizes without returning first?",
    "Yes – Customer Care can send the new size right away and the customer returns the first one within 30 days. Use the *Exchange* button on the order, not a new order.",
  ],
  [
    "do we ship to Switzerland?",
    "Yes, with DHL Express only. Customers pay import VAT on delivery; the checkout shows a note about it. Delivery is usually 2–3 business days.",
  ],
  [
    "wie lange ist die Garantie auf Schlafsäcke?",
    "*2 Jahre*, wie bei allen Produkten außer Zelten und Rucksäcken (die haben 5 Jahre). Abgedeckt sind Nähte, Reißverschlüsse und Materialfehler, nicht der Verschleiß der Daune.",
  ],
  ["what's the cutoff for same-day shipping?", "*15:00 CET* on working days for orders paid by card or PayPal. Bank transfers ship the day the money arrives."],
  [
    "est-ce qu'on accepte les retours en magasin ?",
    "Non, nous n'avons pas de magasins : les retours passent uniquement par l'étiquette prépayée du compte client (gratuite, 30 jours).",
  ],
  [
    "who approves refunds over 150 EUR?",
    "Any Customer Care lead (Noor or Priya). Kaeso's triage marks those tickets with `needs_human: true`, so they land in the leads' queue automatically.",
  ],
  [
    "how do I get a Kaeso key for a new internal tool?",
    "Ask an admin of the *Internal tools* project (Ada or Noor) to add you as a developer; then create a key under *Keys*. Keep it in 1Password, never in the repo.",
  ],
];

const ANNOUNCEMENTS = [
  [
    "new parental leave policy",
    "We're extending paid parental leave to 20 weeks for all parents, starting next month. Details are in the handbook; HR will host a Q&A on Thursday.",
  ],
  [
    "office move",
    "Our Leipzig team moves to the new office on the third floor next Monday. Desks are packed by Friday 15:00; IT will set up screens over the weekend.",
  ],
  [
    "Kaeso usage dashboard",
    "You can now see what each of our AI features costs per day: open Kaeso, pick a project and go to *Usage*. Questions → #ai-platform.",
  ],
  [
    "winter returns extension",
    "Orders placed between now and the end of the year can be returned until the end of January, so gifts can be exchanged after the holidays.",
  ],
  ["on-call rotation change", "From next week, on-call hand-over moves from Monday 09:00 to Tuesday 09:00, so the hand-over doesn't fall on a release day."],
];

// One question to the Slack helper, with its answer.
export function slackQuestion(index) {
  const next = random(0x51ac_0000 + index * 104_729);
  const family = Math.floor(next() * 5);
  if (family === 0) {
    const t = pick(next, THREADS);
    const names = [...NAMES].sort(() => next() - 0.5);
    return { user: `Summarise this thread about ${t.topic} for people who missed it:\n\n${t.lines(names).join("\n")}`, reply: t.summary };
  }
  if (family === 1) {
    const [en, de, fr] = pick(next, TRANSLATIONS);
    const german = next() < 0.6;
    return { user: `Translate for the ${german ? "German" : "French"} shop, keep the tone:\n"${en}"`, reply: german ? `"${de}"` : `"${fr}"` };
  }
  if (family === 2) {
    const [metric, what, sql] = pick(next, METRICS);
    const days = pick(next, [7, 14, 30, 90]);
    return {
      user: `Can you write me a Postgres query for the ${metric} – ${what} – over the last ${days} days?`,
      reply: `Here you go:\n\`\`\`sql\n${sql.replace("{days}", String(days))}\n\`\`\`\nRun it on the read replica (\`analytics-ro\`), not on the primary.`,
    };
  }
  if (family === 3) {
    const [question, answer] = pick(next, POLICY);
    return { user: question, reply: answer };
  }
  const [topic, text] = pick(next, ANNOUNCEMENTS);
  return {
    user: `Make this ${topic} announcement friendlier and shorter for #general:\n\n${text}`,
    reply: `:wave: ${text.split(". ").slice(0, 2).join(". ").replace(/\.$/, "")}. Questions? Reply in the thread :thread:`,
  };
}

let replies;

// The mock provider's scripted answer to a request whose last user message is one of the prompts above.
export function scriptedReply(body, count = { tickets: 6000, questions: 600 }) {
  const text = lastUserText(body);
  if (!text) return undefined;
  if (!replies) {
    replies = new Map();
    for (let i = 0; i < count.tickets; i++) {
      const t = ticket(i);
      replies.set(t.classify.user, t.classify.reply);
      replies.set(t.draft.user, t.draft.reply);
    }
    for (let i = 0; i < count.questions; i++) {
      const q = slackQuestion(i);
      replies.set(q.user, q.reply);
    }
  }
  return replies.get(text);
}

function lastUserText(body) {
  const textOf = (content) =>
    typeof content === "string"
      ? content
      : Array.isArray(content)
        ? content
            .filter((part) => part?.type === "text" || part?.type === "input_text")
            .map((part) => part.text)
            .join("")
        : "";
  if (Array.isArray(body?.messages)) {
    const last = body.messages.findLast((m) => m?.role === "user");
    return last ? textOf(last.content) : "";
  }
  if (typeof body?.input === "string") return body.input;
  if (Array.isArray(body?.input)) {
    const last = body.input.findLast((m) => m?.role === "user");
    return last ? textOf(last.content) : "";
  }
  return "";
}
