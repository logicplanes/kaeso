const areas = import.meta.glob<Record<string, string>>("./de/*.json", { eager: true, import: "default" });
const messages: Readonly<Record<string, string>> = Object.assign({}, ...Object.values(areas));
export default messages;
