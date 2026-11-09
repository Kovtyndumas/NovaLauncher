(function(root){
  function analyze(text,code){
    if(code===0)return null;
    // Classpaths/launch commands mention OpenGL, authentication and mod classes
    // without signalling a failure. Diagnose actual error lines instead.
    const lines=String(text||'').split(/\r?\n/).filter(line=>line.trim()&&!/\[MCLC\]: Launching with arguments|^\s*(?:Class ?path|Command line|JVM Flags|GL debug messages|Backend library|Launched Version|Graphics card)\s*:/i.test(line));
    text=lines.join('\n');
    const rules=[
      [/Требуется вход Horizon|Or sign in with the Microsoft account|No signed-in account owns Minecraft/i,'Требуется вход в Horizon','Для 26.3 RUST нажмите «Войти в Horizon» и подтвердите аккаунт с Minecraft Java Edition. Вход Microsoft в Nova используется отдельно.'],
      [/Unable to initialize SDL:\s*No available video device|GLFW.*(?:first thread|main thread)|Please run.*-XstartOnFirstThread/i,'Не удалось создать окно игры на Mac','На macOS проверьте параметр -XstartOnFirstThread. Он добавляется автоматически в новой версии Nova; перезапустите лаунчер после обновления.'],
      [/UnsupportedClassVersionError|class file version|requires Java|requires.*version.*Java/i,'Неподходящая версия Java','Включите автоматическую Java в настройках и очистите поле собственного пути к Java.'],
      [/OutOfMemoryError|Could not reserve enough space|Cannot allocate memory|insufficient memory/i,'Не хватает памяти','Закройте тяжёлые приложения. При Java heap space увеличьте RAM в настройках; если память не выделяется — уменьшите её.'],
      [/Incompatible mod set|ModResolutionException|requires.*(?:mod|fabric)|Missing.*dependenc|MissingModsException|mandatory dependenc/i,'Несовместимые моды или зависимости','Проверьте версию Minecraft и загрузчик. Установите недостающие зависимости или отключите недавно добавленные моды.'],
      [/Mixin.*(?:failed|error)|MixinApplyError|InvalidMixinException|InjectionError/i,'Конфликт модификаций','Обновите моды под выбранную версию. Если не поможет — отключите последние добавленные моды и проверьте запуск.'],
      [/GLFW error|(?:OpenGL|GLX|WGL).*(?:error|failed|not supported|unavailable)|(?:error|failed|unable).*OpenGL|Pixel format not accelerated|Failed to create window|Unable to create.*context/i,'Ошибка графики','Обновите систему и драйвер видеокарты. Отключите шейдеры и графические моды для проверки.'],
      [/Invalid session|Failed to verify username|AuthenticationException|Unauthorized|not authenticated/i,'Ошибка входа','Выйдите из аккаунта Microsoft и войдите снова. Проверьте подключение к интернету.'],
      [/ENOTFOUND|ETIMEDOUT|ECONNRESET|fetch failed|UnknownHostException/i,'Ошибка сети','Проверьте интернет и повторите запуск. Сервер загрузки может быть временно недоступен.'],
      [/ENOENT.*java|spawn.*java.*ENOENT|Unable to access jarfile|Invalid or corrupt jarfile/i,'Не найден или повреждён файл запуска','Включите автоматическую Java. Проверьте путь к Java и повторите загрузку игры.'],
    ];
    const found=rules.find(([pattern])=>pattern.test(text));
    if(!found&&code===0)return null;

    return {title:found?found[1]:'Игра завершилась с ошибкой',advice:found?found[2]:'Причину не удалось определить автоматически. Откройте консоль и проверьте последние строки лога.',code:code??null,evidence:(found?lines.filter(l=>found[0].test(l)):lines.slice(-8)).slice(0,4).map(x=>x.slice(0,500)),probable:true};
  }
  if(typeof module==='object'&&module.exports)module.exports=analyze;else root.novaAnalyzeCrash=analyze;
})(globalThis);
