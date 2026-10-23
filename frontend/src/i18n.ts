// Copyright 2024 VoteChain Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en.json";

const supportedLanguage = typeof navigator !== "undefined" && navigator.language.toLowerCase().startsWith("en")
  ? "en"
  : "en";

void i18n.use(initReactI18next).init({
  resources: { en: { translation: en } },
  lng: supportedLanguage,
  fallbackLng: "en",
  interpolation: { escapeValue: false }
});

export default i18n;
