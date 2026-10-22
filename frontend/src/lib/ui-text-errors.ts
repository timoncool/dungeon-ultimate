import type { Language } from "@/lib/types";

// Ошибки сервера на языке игры. Сервер шлёт рядом с русским текстом код и поля
// (crates/du-server/src/errors.rs), здесь — шаблон на каждый код. Тест сервера
// `every_code_has_a_translation_in_every_language` проверяет, что у каждого кода есть строка
// во всех семи языках.

type ErrorTexts = Record<string, string>;

const RU: ErrorTexts = {
  job_lost: "задача пропала",
  job_crashed: "задача упала: {detail}",
  turn_aborted: "ход прерван",
  hero_dead: "Герой погиб — история окончена. Отмени последний ход или начни новую историю.",
  narrator_only_artifacts: "нарратор написал только служебные вставки, прозы после их чистки не осталось",
  llm_bin_missing: "не удалось запустить модель: llama-server не найден: {path}",
  llm_model_missing: "не удалось запустить модель: модель не найдена: {path}",
  llm_spawn: "не удалось запустить модель: {detail}",
  llm_network: "сеть: {detail}",
  llm_stream_broken: "сеть: поток оборвался посреди ответа: {detail}",
  llm_key_rejected: "сервер модели отклонил ключ ({status}): {detail}",
  llm_status: "модель ответила ошибкой: {status}: {detail}",
  llm_api: "модель ответила ошибкой: {detail}",
  llm_tool_call: "модель позвала инструмент вместо текста (finish_reason={finish})",
  llm_only_thinking:
    "модель только размышляла ({chars} знаков) и не написала текст (finish_reason={finish}): выключите у неё размышления или выберите модель без них",
  llm_filtered: "провайдер отфильтровал ответ (content_filter)",
  llm_empty: "модель вернула пустой текст (finish_reason={finish})",
  llm_empty_no_reason: "модель вернула пустой текст без причины завершения",
  llm_cut: "поток оборвался, не дав ни текста, ни причины завершения",
  text_model_missing: "не хватает файлов модели: нет модели нарратора ({path}) или llama-server",
  local_server_url_empty: "в настройках истории выбран локальный сервер, но его адрес пуст: впишите адрес сервера",
  agent_not_connected:
    "рассказчик — агент, но агент не подключён к MCP игры: подключите его или выберите рассказчика на своей карте",
  cloud_key_missing:
    "{stage} включён в облаке, но ключ OpenRouter не задан — впишите ключ или выключите облако для этой стадии",
  cloud_model_missing:
    "{stage} включён в облаке, но модель не выбрана — выберите модель или выключите облако для этой стадии",
  cloud_refused: "{stage} в облаке: {detail} ({status})",
  cloud_refused_silent: "{stage} в облаке: провайдер отказал ({status})",
  cloud_network: "{stage} в облаке: {detail}",
  cloud_said: "{stage} в облаке: {detail}",
  cloud_empty: "{stage} в облаке: пустой ответ",
  cloud_too_small: "{stage} в облаке: слишком мало данных ({bytes} байт)",
  google_key_missing:
    "озвучка Google включена, но ключа нет — задайте ключ Google в настройках озвучки или GEMINI_API_KEY",
  openrouter_key_not_set: "ключ OpenRouter не задан",
  voice_missing:
    "голоса «{voice}» нет на диске: поставьте «Набор голосов» в «Что скачать», добавьте этот голос или выберите другой",
  no_voices: "голосов для озвучки нет: поставьте «Набор голосов» в «Что скачать» или добавьте свой голос",
  tts_engine_missing: "движок озвучки не установлен",
  tts_engine_load: "не удалось загрузить движок: {detail}",
  tts_cancelled: "генерация отменена",
  tts_failed: "генерация не удалась: {detail}",
  tts_bad_param: "неверный параметр: {detail}",
  voice_reference: "не удалось подготовить эталон голоса: {detail}",
  tts_empty_text: "пустой текст для озвучки",
  tts_nothing: "нечего озвучивать",
  image_weights_missing: "не хватает файлов модели: нет весов картинок в {path}",
  image_engine_missing: "движок картинок не найден: {path}",
  image_engine_load: "не удалось загрузить движок картинок: {detail}",
  image_bad_path: "в пути к модели недопустимый нулевой байт: {detail}",
  image_context: "движок картинок: не удалось создать контекст модели: {detail}",
  image_failed: "движок картинок: генерация не удалась: {detail}",
  image_no_frame: "движок не вернул кадр",
  asr_failed: "Parakeet не распознал запись: {detail}",
  whisper_bin_missing:
    "выбран Whisper, но его программы нет в {path} — скачайте вариант Whisper в «Что скачать» или выберите Parakeet",
  whisper_model_missing: "выбран Whisper {model}, но модели нет в {path} — скачайте её в «Что скачать»",
  whisper_cuda_missing:
    "Whisper должен считать на видеокарте, но библиотек CUDA для него нет — скачайте «Whisper на видеокарте» в «Что скачать» или переключите распознавание на процессор",
  parakeet_model_missing: "нет модели распознавания {path} — либо скачайте её в «Что скачать», либо включите облако",
  asr_engine_missing: "нет движка распознавания в {path} — скачайте Parakeet в «Что скачать»",
  asr_engine_gpu_missing:
    "распознавание выбрано на видеокарте, а сборки для неё нет в {path} — скачайте «Parakeet на видеокарте» в «Что скачать» или переключите распознавание на процессор",
  parakeet_cuda_lib_missing:
    "Parakeet на видеокарте берёт {name} у движка озвучки, а его нет в {path} — скачайте «Parakeet на видеокарте» в «Что скачать» или переключите распознавание на процессор",
  cpu_floor: "процессор не подходит: {detail}",
  recording_empty: "пустая запись",
  no_audio_file: "в запросе нет звукового файла",
  no_file: "в запросе нет файла",
  chat_not_found: "чат не найден",
  chat_not_created: "чат не создался",
  message_not_found: "сообщение не найдено",
  item_not_found: "предмет не найден",
  item_gone: "предмет пропал",
  quest_not_found: "задание не найдено",
  character_not_found: "персонаж не найден",
  character_name_required: "у персонажа должно быть имя",
  unknown_field: "неизвестное поле: {field}",
  unknown_action: "неизвестное действие: {action}",
  bad_setting: "непонятная настройка: {detail}",
  download_hash_mismatch: "файл скачался повреждённым (контрольная сумма не совпала): {detail}",
  download_network: "не удалось скачать — Hugging Face не отвечает или недоступен: {detail}",
  download_archive: "архив не распаковался: {detail}",
  download_disk: "не удалось записать на диск — проверьте свободное место: {detail}",
  download_other: "не скачалось: {detail}",
};

const EN: ErrorTexts = {
  job_lost: "the task was lost",
  job_crashed: "the task crashed: {detail}",
  turn_aborted: "the turn was stopped",
  hero_dead: "The hero is dead — the story is over. Undo the last turn or start a new story.",
  narrator_only_artifacts: "the narrator wrote only service markup; no prose was left after cleaning it",
  llm_bin_missing: "could not start the model: llama-server not found: {path}",
  llm_model_missing: "could not start the model: model file not found: {path}",
  llm_spawn: "could not start the model: {detail}",
  llm_network: "network: {detail}",
  llm_stream_broken: "network: the stream broke off mid-answer: {detail}",
  llm_key_rejected: "the model server rejected the key ({status}): {detail}",
  llm_status: "the model returned an error: {status}: {detail}",
  llm_api: "the model returned an error: {detail}",
  llm_tool_call: "the model called a tool instead of writing text (finish_reason={finish})",
  llm_only_thinking:
    "the model only reasoned ({chars} characters) and wrote no text (finish_reason={finish}): turn its reasoning off or pick a model without it",
  llm_filtered: "the provider filtered the answer (content_filter)",
  llm_empty: "the model returned empty text (finish_reason={finish})",
  llm_empty_no_reason: "the model returned empty text with no finish reason",
  llm_cut: "the stream broke off with no text and no finish reason",
  text_model_missing: "model files are missing: no narrator model ({path}) or llama-server",
  local_server_url_empty: "the story is set to a local server, but its address is empty: enter the server address",
  agent_not_connected:
    "the narrator is an agent, but no agent is connected to the game's MCP: connect it or choose the narrator on your own card",
  cloud_key_missing:
    "{stage} is set to the cloud, but no OpenRouter key is given — enter the key or turn the cloud off for this stage",
  cloud_model_missing:
    "{stage} is set to the cloud, but no model is chosen — choose a model or turn the cloud off for this stage",
  cloud_refused: "{stage} in the cloud: {detail} ({status})",
  cloud_refused_silent: "{stage} in the cloud: the provider refused ({status})",
  cloud_network: "{stage} in the cloud: {detail}",
  cloud_said: "{stage} in the cloud: {detail}",
  cloud_empty: "{stage} in the cloud: empty answer",
  cloud_too_small: "{stage} in the cloud: too little data ({bytes} bytes)",
  google_key_missing:
    "Google voicing is on, but there is no key — set the Google key in the voice settings or GEMINI_API_KEY",
  openrouter_key_not_set: "no OpenRouter key is set",
  voice_missing:
    "the voice “{voice}” is not on disk: install the “Voice set” in “What to download”, add this voice or choose another",
  no_voices: "there are no voices to speak with: install the “Voice set” in “What to download” or add your own voice",
  tts_engine_missing: "the voice engine is not installed",
  tts_engine_load: "could not load the voice engine: {detail}",
  tts_cancelled: "voicing was cancelled",
  tts_failed: "voicing failed: {detail}",
  tts_bad_param: "wrong voice parameter: {detail}",
  voice_reference: "could not prepare the voice sample: {detail}",
  tts_empty_text: "there is no text to voice",
  tts_nothing: "nothing to voice",
  image_weights_missing: "model files are missing: no image weights in {path}",
  image_engine_missing: "the image engine was not found: {path}",
  image_engine_load: "could not load the image engine: {detail}",
  image_bad_path: "the model path contains a forbidden zero byte: {detail}",
  image_context: "image engine: could not load the model: {detail}",
  image_failed: "image engine: drawing failed: {detail}",
  image_no_frame: "the engine returned no picture",
  asr_failed: "Parakeet could not recognise the recording: {detail}",
  whisper_bin_missing:
    "Whisper is chosen, but its program is not in {path} — download a Whisper variant in “What to download” or choose Parakeet",
  whisper_model_missing: "Whisper {model} is chosen, but the model is not in {path} — download it in “What to download”",
  whisper_cuda_missing:
    "Whisper is set to the graphics card, but its CUDA libraries are missing — download “Whisper on the graphics card” in “What to download” or switch recognition to the processor",
  parakeet_model_missing: "no recognition model at {path} — download it in “What to download” or turn the cloud on",
  asr_engine_missing: "no recognition engine in {path} — download Parakeet in “What to download”",
  asr_engine_gpu_missing:
    "recognition is set to the graphics card, but there is no build for it in {path} — download “Parakeet on the graphics card” in “What to download” or switch recognition to the processor",
  parakeet_cuda_lib_missing:
    "Parakeet on the graphics card takes {name} from the voice engine, and it is not in {path} — download “Parakeet on the graphics card” in “What to download” or switch recognition to the processor",
  cpu_floor: "this processor will not do: {detail}",
  recording_empty: "the recording is empty",
  no_audio_file: "the request has no audio file",
  no_file: "the request has no file",
  chat_not_found: "story not found",
  chat_not_created: "the story was not created",
  message_not_found: "message not found",
  item_not_found: "item not found",
  item_gone: "the item is gone",
  quest_not_found: "quest not found",
  character_not_found: "character not found",
  character_name_required: "a character needs a name",
  unknown_field: "unknown field: {field}",
  unknown_action: "unknown action: {action}",
  bad_setting: "unreadable setting: {detail}",
  download_hash_mismatch: "the file arrived damaged (checksum mismatch): {detail}",
  download_network: "download failed — Hugging Face is not answering or unreachable: {detail}",
  download_archive: "the archive did not unpack: {detail}",
  download_disk: "could not write to disk — check the free space: {detail}",
  download_other: "download failed: {detail}",
};

const ES: ErrorTexts = {
  job_lost: "la tarea se perdió",
  job_crashed: "la tarea falló: {detail}",
  turn_aborted: "el turno se detuvo",
  hero_dead: "El héroe ha muerto: la historia terminó. Deshaz el último turno o empieza una historia nueva.",
  narrator_only_artifacts: "el narrador solo escribió marcas de servicio; tras limpiarlas no quedó texto",
  llm_bin_missing: "no se pudo iniciar el modelo: no se encontró llama-server: {path}",
  llm_model_missing: "no se pudo iniciar el modelo: no se encontró el archivo del modelo: {path}",
  llm_spawn: "no se pudo iniciar el modelo: {detail}",
  llm_network: "red: {detail}",
  llm_stream_broken: "red: el flujo se cortó a mitad de la respuesta: {detail}",
  llm_key_rejected: "el servidor del modelo rechazó la clave ({status}): {detail}",
  llm_status: "el modelo respondió con un error: {status}: {detail}",
  llm_api: "el modelo respondió con un error: {detail}",
  llm_tool_call: "el modelo llamó a una herramienta en vez de escribir texto (finish_reason={finish})",
  llm_only_thinking:
    "el modelo solo razonó ({chars} caracteres) y no escribió texto (finish_reason={finish}): desactiva su razonamiento o elige un modelo sin él",
  llm_filtered: "el proveedor filtró la respuesta (content_filter)",
  llm_empty: "el modelo devolvió texto vacío (finish_reason={finish})",
  llm_empty_no_reason: "el modelo devolvió texto vacío sin motivo de cierre",
  llm_cut: "el flujo se cortó sin texto ni motivo de cierre",
  text_model_missing: "faltan archivos del modelo: no hay modelo del narrador ({path}) o llama-server",
  local_server_url_empty:
    "la historia usa un servidor local, pero su dirección está vacía: escribe la dirección del servidor",
  agent_not_connected:
    "el narrador es un agente, pero no hay ningún agente conectado al MCP del juego: conéctalo o elige el narrador en tu propia tarjeta",
  cloud_key_missing:
    "{stage} está en la nube, pero falta la clave de OpenRouter: escribe la clave o desactiva la nube para esta etapa",
  cloud_model_missing:
    "{stage} está en la nube, pero no hay modelo elegido: elige un modelo o desactiva la nube para esta etapa",
  cloud_refused: "{stage} en la nube: {detail} ({status})",
  cloud_refused_silent: "{stage} en la nube: el proveedor se negó ({status})",
  cloud_network: "{stage} en la nube: {detail}",
  cloud_said: "{stage} en la nube: {detail}",
  cloud_empty: "{stage} en la nube: respuesta vacía",
  cloud_too_small: "{stage} en la nube: muy pocos datos ({bytes} bytes)",
  google_key_missing:
    "la voz de Google está activada, pero no hay clave: pon la clave de Google en los ajustes de voz o GEMINI_API_KEY",
  openrouter_key_not_set: "no hay clave de OpenRouter",
  voice_missing:
    "la voz «{voice}» no está en el disco: instala el «Juego de voces» en «Qué descargar», añade esta voz o elige otra",
  no_voices: "no hay voces para hablar: instala el «Juego de voces» en «Qué descargar» o añade tu propia voz",
  tts_engine_missing: "el motor de voz no está instalado",
  tts_engine_load: "no se pudo cargar el motor de voz: {detail}",
  tts_cancelled: "la voz se canceló",
  tts_failed: "la voz falló: {detail}",
  tts_bad_param: "parámetro de voz incorrecto: {detail}",
  voice_reference: "no se pudo preparar la muestra de voz: {detail}",
  tts_empty_text: "no hay texto que leer",
  tts_nothing: "no hay nada que leer",
  image_weights_missing: "faltan archivos del modelo: no hay pesos de imágenes en {path}",
  image_engine_missing: "no se encontró el motor de imágenes: {path}",
  image_engine_load: "no se pudo cargar el motor de imágenes: {detail}",
  image_bad_path: "la ruta del modelo contiene un byte nulo no permitido: {detail}",
  image_context: "motor de imágenes: no se pudo cargar el modelo: {detail}",
  image_failed: "motor de imágenes: el dibujo falló: {detail}",
  image_no_frame: "el motor no devolvió ninguna imagen",
  asr_failed: "Parakeet no pudo reconocer la grabación: {detail}",
  whisper_bin_missing:
    "Whisper está elegido, pero su programa no está en {path}: descarga una variante de Whisper en «Qué descargar» o elige Parakeet",
  whisper_model_missing: "Whisper {model} está elegido, pero el modelo no está en {path}: descárgalo en «Qué descargar»",
  whisper_cuda_missing:
    "Whisper debe usar la tarjeta gráfica, pero faltan sus bibliotecas CUDA: descarga «Whisper en la tarjeta gráfica» en «Qué descargar» o pasa el reconocimiento al procesador",
  parakeet_model_missing: "no hay modelo de reconocimiento en {path}: descárgalo en «Qué descargar» o activa la nube",
  asr_engine_missing: "no hay motor de reconocimiento en {path}: descarga Parakeet en «Qué descargar»",
  asr_engine_gpu_missing:
    "el reconocimiento está en la tarjeta gráfica, pero no hay versión para ella en {path}: descarga «Parakeet en la tarjeta gráfica» en «Qué descargar» o pasa el reconocimiento al procesador",
  parakeet_cuda_lib_missing:
    "Parakeet en la tarjeta gráfica toma {name} del motor de voz, y no está en {path}: descarga «Parakeet en la tarjeta gráfica» en «Qué descargar» o pasa el reconocimiento al procesador",
  cpu_floor: "este procesador no sirve: {detail}",
  recording_empty: "la grabación está vacía",
  no_audio_file: "la solicitud no trae archivo de audio",
  no_file: "la solicitud no trae archivo",
  chat_not_found: "historia no encontrada",
  chat_not_created: "la historia no se creó",
  message_not_found: "mensaje no encontrado",
  item_not_found: "objeto no encontrado",
  item_gone: "el objeto ya no está",
  quest_not_found: "misión no encontrada",
  character_not_found: "personaje no encontrado",
  character_name_required: "el personaje necesita un nombre",
  unknown_field: "campo desconocido: {field}",
  unknown_action: "acción desconocida: {action}",
  bad_setting: "ajuste ilegible: {detail}",
  download_hash_mismatch: "el archivo llegó dañado (la suma de control no coincide): {detail}",
  download_network: "la descarga falló: Hugging Face no responde o no es accesible: {detail}",
  download_archive: "el archivo comprimido no se descomprimió: {detail}",
  download_disk: "no se pudo escribir en el disco: revisa el espacio libre: {detail}",
  download_other: "la descarga falló: {detail}",
};

const FR: ErrorTexts = {
  job_lost: "la tâche a été perdue",
  job_crashed: "la tâche a planté : {detail}",
  turn_aborted: "le tour a été interrompu",
  hero_dead: "Le héros est mort — l’histoire est finie. Annule le dernier tour ou commence une nouvelle histoire.",
  narrator_only_artifacts: "le narrateur n’a écrit que du balisage de service ; une fois nettoyé, il ne reste aucun texte",
  llm_bin_missing: "impossible de lancer le modèle : llama-server introuvable : {path}",
  llm_model_missing: "impossible de lancer le modèle : fichier du modèle introuvable : {path}",
  llm_spawn: "impossible de lancer le modèle : {detail}",
  llm_network: "réseau : {detail}",
  llm_stream_broken: "réseau : le flux s’est coupé au milieu de la réponse : {detail}",
  llm_key_rejected: "le serveur du modèle a refusé la clé ({status}) : {detail}",
  llm_status: "le modèle a répondu par une erreur : {status} : {detail}",
  llm_api: "le modèle a répondu par une erreur : {detail}",
  llm_tool_call: "le modèle a appelé un outil au lieu d’écrire du texte (finish_reason={finish})",
  llm_only_thinking:
    "le modèle n’a fait que réfléchir ({chars} caractères) sans écrire de texte (finish_reason={finish}) : désactive sa réflexion ou choisis un modèle sans",
  llm_filtered: "le fournisseur a filtré la réponse (content_filter)",
  llm_empty: "le modèle a renvoyé un texte vide (finish_reason={finish})",
  llm_empty_no_reason: "le modèle a renvoyé un texte vide sans raison de fin",
  llm_cut: "le flux s’est coupé sans texte ni raison de fin",
  text_model_missing: "fichiers du modèle manquants : pas de modèle du narrateur ({path}) ou de llama-server",
  local_server_url_empty:
    "l’histoire utilise un serveur local, mais son adresse est vide : saisis l’adresse du serveur",
  agent_not_connected:
    "le narrateur est un agent, mais aucun agent n’est connecté au MCP du jeu : connecte-le ou choisis le narrateur sur ta propre carte",
  cloud_key_missing:
    "{stage} est dans le nuage, mais la clé OpenRouter manque — saisis la clé ou désactive le nuage pour cette étape",
  cloud_model_missing:
    "{stage} est dans le nuage, mais aucun modèle n’est choisi — choisis un modèle ou désactive le nuage pour cette étape",
  cloud_refused: "{stage} dans le nuage : {detail} ({status})",
  cloud_refused_silent: "{stage} dans le nuage : le fournisseur a refusé ({status})",
  cloud_network: "{stage} dans le nuage : {detail}",
  cloud_said: "{stage} dans le nuage : {detail}",
  cloud_empty: "{stage} dans le nuage : réponse vide",
  cloud_too_small: "{stage} dans le nuage : trop peu de données ({bytes} octets)",
  google_key_missing:
    "la voix Google est activée, mais il n’y a pas de clé — mets la clé Google dans les réglages de voix ou GEMINI_API_KEY",
  openrouter_key_not_set: "aucune clé OpenRouter",
  voice_missing:
    "la voix « {voice} » n’est pas sur le disque : installe le « Jeu de voix » dans « Que télécharger », ajoute cette voix ou choisis-en une autre",
  no_voices: "aucune voix pour parler : installe le « Jeu de voix » dans « Que télécharger » ou ajoute ta propre voix",
  tts_engine_missing: "le moteur de voix n’est pas installé",
  tts_engine_load: "impossible de charger le moteur de voix : {detail}",
  tts_cancelled: "la voix a été annulée",
  tts_failed: "la voix a échoué : {detail}",
  tts_bad_param: "paramètre de voix incorrect : {detail}",
  voice_reference: "impossible de préparer l’échantillon de voix : {detail}",
  tts_empty_text: "aucun texte à lire",
  tts_nothing: "rien à lire",
  image_weights_missing: "fichiers du modèle manquants : pas de poids d’images dans {path}",
  image_engine_missing: "moteur d’images introuvable : {path}",
  image_engine_load: "impossible de charger le moteur d’images : {detail}",
  image_bad_path: "le chemin du modèle contient un octet nul interdit : {detail}",
  image_context: "moteur d’images : impossible de charger le modèle : {detail}",
  image_failed: "moteur d’images : le dessin a échoué : {detail}",
  image_no_frame: "le moteur n’a renvoyé aucune image",
  asr_failed: "Parakeet n’a pas pu reconnaître l’enregistrement : {detail}",
  whisper_bin_missing:
    "Whisper est choisi, mais son programme n’est pas dans {path} — télécharge une variante de Whisper dans « Que télécharger » ou choisis Parakeet",
  whisper_model_missing:
    "Whisper {model} est choisi, mais le modèle n’est pas dans {path} — télécharge-le dans « Que télécharger »",
  whisper_cuda_missing:
    "Whisper doit utiliser la carte graphique, mais ses bibliothèques CUDA manquent — télécharge « Whisper sur la carte graphique » dans « Que télécharger » ou passe la reconnaissance sur le processeur",
  parakeet_model_missing:
    "pas de modèle de reconnaissance dans {path} — télécharge-le dans « Que télécharger » ou active le nuage",
  asr_engine_missing: "pas de moteur de reconnaissance dans {path} — télécharge Parakeet dans « Que télécharger »",
  asr_engine_gpu_missing:
    "la reconnaissance est réglée sur la carte graphique, mais il n’y a pas de version pour elle dans {path} — télécharge « Parakeet sur la carte graphique » dans « Que télécharger » ou passe la reconnaissance sur le processeur",
  parakeet_cuda_lib_missing:
    "Parakeet sur la carte graphique prend {name} au moteur de voix, et il n’est pas dans {path} — télécharge « Parakeet sur la carte graphique » dans « Que télécharger » ou passe la reconnaissance sur le processeur",
  cpu_floor: "ce processeur ne convient pas : {detail}",
  recording_empty: "l’enregistrement est vide",
  no_audio_file: "la requête ne contient pas de fichier audio",
  no_file: "la requête ne contient pas de fichier",
  chat_not_found: "histoire introuvable",
  chat_not_created: "l’histoire n’a pas été créée",
  message_not_found: "message introuvable",
  item_not_found: "objet introuvable",
  item_gone: "l’objet a disparu",
  quest_not_found: "quête introuvable",
  character_not_found: "personnage introuvable",
  character_name_required: "un personnage doit avoir un nom",
  unknown_field: "champ inconnu : {field}",
  unknown_action: "action inconnue : {action}",
  bad_setting: "réglage illisible : {detail}",
  download_hash_mismatch: "le fichier est arrivé abîmé (somme de contrôle différente) : {detail}",
  download_network: "téléchargement échoué — Hugging Face ne répond pas ou est inaccessible : {detail}",
  download_archive: "l’archive ne s’est pas décompressée : {detail}",
  download_disk: "impossible d’écrire sur le disque — vérifie l’espace libre : {detail}",
  download_other: "téléchargement échoué : {detail}",
};

const DE: ErrorTexts = {
  job_lost: "die Aufgabe ist verloren gegangen",
  job_crashed: "die Aufgabe ist abgestürzt: {detail}",
  turn_aborted: "der Zug wurde abgebrochen",
  hero_dead: "Der Held ist tot — die Geschichte ist zu Ende. Mach den letzten Zug rückgängig oder beginne eine neue Geschichte.",
  narrator_only_artifacts: "der Erzähler schrieb nur Dienst-Markierungen; nach dem Bereinigen blieb kein Text übrig",
  llm_bin_missing: "das Modell ließ sich nicht starten: llama-server nicht gefunden: {path}",
  llm_model_missing: "das Modell ließ sich nicht starten: Modelldatei nicht gefunden: {path}",
  llm_spawn: "das Modell ließ sich nicht starten: {detail}",
  llm_network: "Netzwerk: {detail}",
  llm_stream_broken: "Netzwerk: der Datenstrom brach mitten in der Antwort ab: {detail}",
  llm_key_rejected: "der Modellserver hat den Schlüssel abgelehnt ({status}): {detail}",
  llm_status: "das Modell antwortete mit einem Fehler: {status}: {detail}",
  llm_api: "das Modell antwortete mit einem Fehler: {detail}",
  llm_tool_call: "das Modell hat ein Werkzeug aufgerufen, statt Text zu schreiben (finish_reason={finish})",
  llm_only_thinking:
    "das Modell hat nur nachgedacht ({chars} Zeichen) und keinen Text geschrieben (finish_reason={finish}): schalte sein Nachdenken ab oder wähle ein Modell ohne",
  llm_filtered: "der Anbieter hat die Antwort gefiltert (content_filter)",
  llm_empty: "das Modell lieferte leeren Text (finish_reason={finish})",
  llm_empty_no_reason: "das Modell lieferte leeren Text ohne Abschlussgrund",
  llm_cut: "der Datenstrom brach ab, ohne Text und ohne Abschlussgrund",
  text_model_missing: "Modelldateien fehlen: kein Erzählermodell ({path}) oder kein llama-server",
  local_server_url_empty:
    "die Geschichte nutzt einen lokalen Server, aber seine Adresse ist leer: trage die Serveradresse ein",
  agent_not_connected:
    "der Erzähler ist ein Agent, aber mit dem MCP des Spiels ist kein Agent verbunden: verbinde ihn oder wähle den Erzähler auf deiner eigenen Karte",
  cloud_key_missing:
    "{stage} läuft in der Cloud, aber es fehlt der OpenRouter-Schlüssel — trage ihn ein oder schalte die Cloud für diese Stufe ab",
  cloud_model_missing:
    "{stage} läuft in der Cloud, aber kein Modell ist gewählt — wähle ein Modell oder schalte die Cloud für diese Stufe ab",
  cloud_refused: "{stage} in der Cloud: {detail} ({status})",
  cloud_refused_silent: "{stage} in der Cloud: der Anbieter hat abgelehnt ({status})",
  cloud_network: "{stage} in der Cloud: {detail}",
  cloud_said: "{stage} in der Cloud: {detail}",
  cloud_empty: "{stage} in der Cloud: leere Antwort",
  cloud_too_small: "{stage} in der Cloud: zu wenig Daten ({bytes} Bytes)",
  google_key_missing:
    "die Google-Stimme ist an, aber es gibt keinen Schlüssel — setze den Google-Schlüssel in den Stimmeinstellungen oder GEMINI_API_KEY",
  openrouter_key_not_set: "kein OpenRouter-Schlüssel gesetzt",
  voice_missing:
    "die Stimme „{voice}“ ist nicht auf der Festplatte: installiere das „Stimmen-Set“ unter „Was herunterladen“, füge diese Stimme hinzu oder wähle eine andere",
  no_voices: "keine Stimmen zum Sprechen: installiere das „Stimmen-Set“ unter „Was herunterladen“ oder füge eine eigene Stimme hinzu",
  tts_engine_missing: "die Sprach-Engine ist nicht installiert",
  tts_engine_load: "die Sprach-Engine ließ sich nicht laden: {detail}",
  tts_cancelled: "die Vertonung wurde abgebrochen",
  tts_failed: "die Vertonung ist fehlgeschlagen: {detail}",
  tts_bad_param: "falscher Stimmparameter: {detail}",
  voice_reference: "die Stimmprobe ließ sich nicht vorbereiten: {detail}",
  tts_empty_text: "kein Text zum Vorlesen",
  tts_nothing: "nichts zum Vorlesen",
  image_weights_missing: "Modelldateien fehlen: keine Bildgewichte in {path}",
  image_engine_missing: "die Bild-Engine wurde nicht gefunden: {path}",
  image_engine_load: "die Bild-Engine ließ sich nicht laden: {detail}",
  image_bad_path: "der Modellpfad enthält ein verbotenes Nullbyte: {detail}",
  image_context: "Bild-Engine: das Modell ließ sich nicht laden: {detail}",
  image_failed: "Bild-Engine: das Zeichnen ist fehlgeschlagen: {detail}",
  image_no_frame: "die Engine hat kein Bild geliefert",
  asr_failed: "Parakeet konnte die Aufnahme nicht erkennen: {detail}",
  whisper_bin_missing:
    "Whisper ist gewählt, aber sein Programm liegt nicht in {path} — lade eine Whisper-Variante unter „Was herunterladen“ oder wähle Parakeet",
  whisper_model_missing:
    "Whisper {model} ist gewählt, aber das Modell liegt nicht in {path} — lade es unter „Was herunterladen“",
  whisper_cuda_missing:
    "Whisper soll auf der Grafikkarte rechnen, aber seine CUDA-Bibliotheken fehlen — lade „Whisper auf der Grafikkarte“ unter „Was herunterladen“ oder stelle die Erkennung auf den Prozessor um",
  parakeet_model_missing:
    "kein Erkennungsmodell in {path} — lade es unter „Was herunterladen“ oder schalte die Cloud ein",
  asr_engine_missing: "keine Erkennungs-Engine in {path} — lade Parakeet unter „Was herunterladen“",
  asr_engine_gpu_missing:
    "die Erkennung ist auf die Grafikkarte gestellt, aber dafür gibt es in {path} keinen Build — lade „Parakeet auf der Grafikkarte“ unter „Was herunterladen“ oder stelle die Erkennung auf den Prozessor um",
  parakeet_cuda_lib_missing:
    "Parakeet auf der Grafikkarte nimmt {name} von der Sprach-Engine, und es fehlt in {path} — lade „Parakeet auf der Grafikkarte“ unter „Was herunterladen“ oder stelle die Erkennung auf den Prozessor um",
  cpu_floor: "dieser Prozessor reicht nicht: {detail}",
  recording_empty: "die Aufnahme ist leer",
  no_audio_file: "die Anfrage enthält keine Audiodatei",
  no_file: "die Anfrage enthält keine Datei",
  chat_not_found: "Geschichte nicht gefunden",
  chat_not_created: "die Geschichte wurde nicht angelegt",
  message_not_found: "Nachricht nicht gefunden",
  item_not_found: "Gegenstand nicht gefunden",
  item_gone: "der Gegenstand ist verschwunden",
  quest_not_found: "Auftrag nicht gefunden",
  character_not_found: "Figur nicht gefunden",
  character_name_required: "eine Figur braucht einen Namen",
  unknown_field: "unbekanntes Feld: {field}",
  unknown_action: "unbekannte Aktion: {action}",
  bad_setting: "unlesbare Einstellung: {detail}",
  download_hash_mismatch: "die Datei kam beschädigt an (Prüfsumme stimmt nicht): {detail}",
  download_network: "Download fehlgeschlagen — Hugging Face antwortet nicht oder ist nicht erreichbar: {detail}",
  download_archive: "das Archiv ließ sich nicht entpacken: {detail}",
  download_disk: "auf die Festplatte ließ sich nicht schreiben — prüfe den freien Speicher: {detail}",
  download_other: "Download fehlgeschlagen: {detail}",
};

const ZH: ErrorTexts = {
  job_lost: "任务丢失了",
  job_crashed: "任务崩溃：{detail}",
  turn_aborted: "回合已停止",
  hero_dead: "英雄已死，故事结束。撤销上一回合或开始新的故事。",
  narrator_only_artifacts: "讲述者只写了服务标记，清理后没有剩下正文",
  llm_bin_missing: "无法启动模型：找不到 llama-server：{path}",
  llm_model_missing: "无法启动模型：找不到模型文件：{path}",
  llm_spawn: "无法启动模型：{detail}",
  llm_network: "网络：{detail}",
  llm_stream_broken: "网络：回答中途数据流中断：{detail}",
  llm_key_rejected: "模型服务器拒绝了密钥（{status}）：{detail}",
  llm_status: "模型返回错误：{status}：{detail}",
  llm_api: "模型返回错误：{detail}",
  llm_tool_call: "模型调用了工具而没有写正文（finish_reason={finish}）",
  llm_only_thinking:
    "模型只在思考（{chars} 个字符），没有写出正文（finish_reason={finish}）：关闭它的思考，或换一个不带思考的模型",
  llm_filtered: "服务商过滤了回答（content_filter）",
  llm_empty: "模型返回了空文本（finish_reason={finish}）",
  llm_empty_no_reason: "模型返回了空文本，没有结束原因",
  llm_cut: "数据流中断，既没有文本也没有结束原因",
  text_model_missing: "缺少模型文件：没有讲述者模型（{path}）或 llama-server",
  local_server_url_empty: "故事设置为本地服务器，但地址为空：请填写服务器地址",
  agent_not_connected: "讲述者是智能体，但没有智能体连接到游戏的 MCP：请连接它，或选择在本机显卡上运行讲述者",
  cloud_key_missing: "{stage}设为云端，但未填写 OpenRouter 密钥：请填写密钥，或为此阶段关闭云端",
  cloud_model_missing: "{stage}设为云端，但未选择模型：请选择模型，或为此阶段关闭云端",
  cloud_refused: "云端{stage}：{detail}（{status}）",
  cloud_refused_silent: "云端{stage}：服务商拒绝了请求（{status}）",
  cloud_network: "云端{stage}：{detail}",
  cloud_said: "云端{stage}：{detail}",
  cloud_empty: "云端{stage}：返回为空",
  cloud_too_small: "云端{stage}：数据太少（{bytes} 字节）",
  google_key_missing: "已开启 Google 配音，但没有密钥：请在配音设置中填写 Google 密钥或设置 GEMINI_API_KEY",
  openrouter_key_not_set: "未设置 OpenRouter 密钥",
  voice_missing: "磁盘上没有声音“{voice}”：请在“下载什么”中安装“声音包”，添加这个声音或换一个",
  no_voices: "没有可用的声音：请在“下载什么”中安装“声音包”，或添加你自己的声音",
  tts_engine_missing: "未安装配音引擎",
  tts_engine_load: "无法加载配音引擎：{detail}",
  tts_cancelled: "配音已取消",
  tts_failed: "配音失败：{detail}",
  tts_bad_param: "配音参数错误：{detail}",
  voice_reference: "无法准备声音样本：{detail}",
  tts_empty_text: "没有要朗读的文本",
  tts_nothing: "没有可朗读的内容",
  image_weights_missing: "缺少模型文件：{path} 中没有图像权重",
  image_engine_missing: "找不到图像引擎：{path}",
  image_engine_load: "无法加载图像引擎：{detail}",
  image_bad_path: "模型路径中含有不允许的空字节：{detail}",
  image_context: "图像引擎：无法加载模型：{detail}",
  image_failed: "图像引擎：绘制失败：{detail}",
  image_no_frame: "引擎没有返回图片",
  asr_failed: "Parakeet 无法识别录音：{detail}",
  whisper_bin_missing: "已选择 Whisper，但 {path} 中没有它的程序：请在“下载什么”中下载 Whisper，或选择 Parakeet",
  whisper_model_missing: "已选择 Whisper {model}，但 {path} 中没有模型：请在“下载什么”中下载",
  whisper_cuda_missing:
    "Whisper 需要在显卡上运行，但缺少它的 CUDA 库：请在“下载什么”中下载“显卡版 Whisper”，或把识别切换到处理器",
  parakeet_model_missing: "{path} 中没有识别模型：请在“下载什么”中下载，或开启云端",
  asr_engine_missing: "{path} 中没有识别引擎：请在“下载什么”中下载 Parakeet",
  asr_engine_gpu_missing:
    "识别设为显卡，但 {path} 中没有显卡版本：请在“下载什么”中下载“显卡版 Parakeet”，或把识别切换到处理器",
  parakeet_cuda_lib_missing:
    "显卡版 Parakeet 需要配音引擎中的 {name}，但 {path} 中没有：请在“下载什么”中下载“显卡版 Parakeet”，或把识别切换到处理器",
  cpu_floor: "处理器不满足要求：{detail}",
  recording_empty: "录音为空",
  no_audio_file: "请求中没有音频文件",
  no_file: "请求中没有文件",
  chat_not_found: "找不到故事",
  chat_not_created: "故事没有创建成功",
  message_not_found: "找不到消息",
  item_not_found: "找不到物品",
  item_gone: "物品已经不见了",
  quest_not_found: "找不到任务",
  character_not_found: "找不到角色",
  character_name_required: "角色必须有名字",
  unknown_field: "未知字段：{field}",
  unknown_action: "未知操作：{action}",
  bad_setting: "无法读取的设置：{detail}",
  download_hash_mismatch: "文件下载后已损坏（校验和不一致）：{detail}",
  download_network: "下载失败：Hugging Face 无响应或无法访问：{detail}",
  download_archive: "压缩包解压失败：{detail}",
  download_disk: "无法写入磁盘，请检查剩余空间：{detail}",
  download_other: "下载失败：{detail}",
};

const JA: ErrorTexts = {
  job_lost: "タスクが見失われた",
  job_crashed: "タスクが落ちた：{detail}",
  turn_aborted: "ターンを中断した",
  hero_dead: "主人公は死んだ。物語はここまで。最後のターンを取り消すか、新しい物語を始めて。",
  narrator_only_artifacts: "語り手がサービス用の記号しか書かず、取り除くと本文が残らなかった",
  llm_bin_missing: "モデルを起動できない：llama-server が見つからない：{path}",
  llm_model_missing: "モデルを起動できない：モデルファイルが見つからない：{path}",
  llm_spawn: "モデルを起動できない：{detail}",
  llm_network: "ネットワーク：{detail}",
  llm_stream_broken: "ネットワーク：回答の途中でストリームが途切れた：{detail}",
  llm_key_rejected: "モデルのサーバーがキーを拒否した（{status}）：{detail}",
  llm_status: "モデルがエラーを返した：{status}：{detail}",
  llm_api: "モデルがエラーを返した：{detail}",
  llm_tool_call: "モデルが本文を書かずにツールを呼んだ（finish_reason={finish}）",
  llm_only_thinking:
    "モデルが考えるだけで（{chars} 文字）本文を書かなかった（finish_reason={finish}）：思考をオフにするか、思考しないモデルを選んで",
  llm_filtered: "プロバイダーが回答をフィルターした（content_filter）",
  llm_empty: "モデルが空の本文を返した（finish_reason={finish}）",
  llm_empty_no_reason: "モデルが終了理由なしで空の本文を返した",
  llm_cut: "本文も終了理由もないままストリームが途切れた",
  text_model_missing: "モデルのファイルが足りない：語り手のモデル（{path}）か llama-server がない",
  local_server_url_empty: "物語はローカルサーバーを使う設定だが、アドレスが空：サーバーのアドレスを入力して",
  agent_not_connected:
    "語り手はエージェントだが、ゲームの MCP にエージェントがつながっていない：つなぐか、手元のカードで動く語り手を選んで",
  cloud_key_missing:
    "{stage}はクラウドの設定だが、OpenRouter のキーがない：キーを入力するか、この段階のクラウドをオフにして",
  cloud_model_missing:
    "{stage}はクラウドの設定だが、モデルが選ばれていない：モデルを選ぶか、この段階のクラウドをオフにして",
  cloud_refused: "クラウドの{stage}：{detail}（{status}）",
  cloud_refused_silent: "クラウドの{stage}：プロバイダーが拒否した（{status}）",
  cloud_network: "クラウドの{stage}：{detail}",
  cloud_said: "クラウドの{stage}：{detail}",
  cloud_empty: "クラウドの{stage}：返事が空",
  cloud_too_small: "クラウドの{stage}：データが少なすぎる（{bytes} バイト）",
  google_key_missing:
    "Google の音声がオンだがキーがない：音声の設定で Google のキーを入力するか GEMINI_API_KEY を設定して",
  openrouter_key_not_set: "OpenRouter のキーが設定されていない",
  voice_missing:
    "声「{voice}」がディスクにない：「何を落とすか」で「声のセット」を入れるか、この声を追加するか、別の声を選んで",
  no_voices: "読み上げに使える声がない：「何を落とすか」で「声のセット」を入れるか、自分の声を追加して",
  tts_engine_missing: "音声エンジンが入っていない",
  tts_engine_load: "音声エンジンを読み込めない：{detail}",
  tts_cancelled: "読み上げを取り消した",
  tts_failed: "読み上げに失敗した：{detail}",
  tts_bad_param: "音声のパラメーターが正しくない：{detail}",
  voice_reference: "声のサンプルを用意できない：{detail}",
  tts_empty_text: "読み上げる文がない",
  tts_nothing: "読み上げるものがない",
  image_weights_missing: "モデルのファイルが足りない：{path} に画像の重みがない",
  image_engine_missing: "画像エンジンが見つからない：{path}",
  image_engine_load: "画像エンジンを読み込めない：{detail}",
  image_bad_path: "モデルのパスに使えないヌル文字がある：{detail}",
  image_context: "画像エンジン：モデルを読み込めない：{detail}",
  image_failed: "画像エンジン：描画に失敗した：{detail}",
  image_no_frame: "エンジンが画像を返さなかった",
  asr_failed: "Parakeet が録音を認識できなかった：{detail}",
  whisper_bin_missing:
    "Whisper を選んでいるが、{path} にプログラムがない：「何を落とすか」で Whisper を落とすか、Parakeet を選んで",
  whisper_model_missing: "Whisper {model} を選んでいるが、{path} にモデルがない：「何を落とすか」で落として",
  whisper_cuda_missing:
    "Whisper はグラフィックカードで動かす設定だが、CUDA ライブラリがない：「何を落とすか」で「グラフィックカード版 Whisper」を落とすか、認識をプロセッサーに切り替えて",
  parakeet_model_missing: "{path} に認識モデルがない：「何を落とすか」で落とすか、クラウドをオンにして",
  asr_engine_missing: "{path} に認識エンジンがない：「何を落とすか」で Parakeet を落として",
  asr_engine_gpu_missing:
    "認識はグラフィックカードの設定だが、{path} にそのためのビルドがない：「何を落とすか」で「グラフィックカード版 Parakeet」を落とすか、認識をプロセッサーに切り替えて",
  parakeet_cuda_lib_missing:
    "グラフィックカード版 Parakeet は音声エンジンの {name} を使うが、{path} にない：「何を落とすか」で「グラフィックカード版 Parakeet」を落とすか、認識をプロセッサーに切り替えて",
  cpu_floor: "このプロセッサーでは動かない：{detail}",
  recording_empty: "録音が空",
  no_audio_file: "リクエストに音声ファイルがない",
  no_file: "リクエストにファイルがない",
  chat_not_found: "物語が見つからない",
  chat_not_created: "物語を作れなかった",
  message_not_found: "メッセージが見つからない",
  item_not_found: "アイテムが見つからない",
  item_gone: "アイテムがなくなった",
  quest_not_found: "依頼が見つからない",
  character_not_found: "キャラクターが見つからない",
  character_name_required: "キャラクターには名前が要る",
  unknown_field: "不明な項目：{field}",
  unknown_action: "不明な操作：{action}",
  bad_setting: "読めない設定：{detail}",
  download_hash_mismatch: "ファイルが壊れて届いた（チェックサムが合わない）：{detail}",
  download_network: "ダウンロードに失敗した：Hugging Face が応答しないか、つながらない：{detail}",
  download_archive: "アーカイブを展開できなかった：{detail}",
  download_disk: "ディスクに書き込めない：空き容量を確かめて：{detail}",
  download_other: "ダウンロードに失敗した：{detail}",
};

// Стадия, о которой говорит ошибка облака, — подставляется вместо {stage}.
const STAGES: Record<Language, Record<string, string>> = {
  ru: { narrator: "рассказчик", image: "картинки", tts: "озвучка", asr: "распознавание речи" },
  en: { narrator: "the narrator", image: "pictures", tts: "voicing", asr: "speech recognition" },
  es: { narrator: "el narrador", image: "las imágenes", tts: "la voz", asr: "el reconocimiento de voz" },
  fr: { narrator: "le narrateur", image: "les images", tts: "la voix", asr: "la reconnaissance vocale" },
  de: { narrator: "der Erzähler", image: "die Bilder", tts: "die Vertonung", asr: "die Spracherkennung" },
  zh: { narrator: "讲述者", image: "图片", tts: "配音", asr: "语音识别" },
  ja: { narrator: "語り手", image: "画像", tts: "読み上げ", asr: "音声認識" },
};

const BY_LANGUAGE: Record<Language, ErrorTexts> = { ru: RU, en: EN, es: ES, fr: FR, de: DE, zh: ZH, ja: JA };

/// Ошибка с сервера, как она пришла: русский текст и, если у неё есть вид, код с полями.
export type ServerError = {
  error?: string;
  code?: string | null;
  params?: Record<string, unknown> | null;
};

let current: Language = "ru";

/// Язык, на котором окно сейчас говорит: его ставит провайдер подписей (`UiTextProvider`).
export function setErrorLanguage(language: Language): void {
  current = language;
}

/// Текст ошибки на языке игры. Без кода — текст сервера как есть: видно, что перевода нет.
export function errorText(payload: ServerError | null | undefined, language: Language = current): string {
  const raw = payload?.error ?? "";
  const code = payload?.code;
  if (!code || code === "raw") return raw;
  const lang = language;
  const template = BY_LANGUAGE[lang][code];
  if (!template) return raw;
  const params = payload?.params ?? {};
  return template.replace(/\{(\w+)\}/g, (match, key: string) => {
    const value = params[key];
    if (value === undefined || value === null) return match;
    if (key === "stage") return STAGES[lang][String(value)] ?? String(value);
    return String(value);
  });
}
