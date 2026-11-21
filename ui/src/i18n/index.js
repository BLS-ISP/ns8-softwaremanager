import axios from "axios";

export async function loadLanguageAsync(lang, i18n) {
  if (i18n.locale === lang && Object.keys(i18n.getLocaleMessage(lang) || {}).length > 0) {
    return lang;
  }
  try {
    const response = await axios.get(`./i18n/${lang}/translation.json`);
    i18n.setLocaleMessage(lang, response.data);
    i18n.locale = lang;
    return lang;
  } catch (error) {
    console.warn(`Cannot load language ${lang}, falling back to en`, error);
    i18n.locale = "en";
    return "en";
  }
}
