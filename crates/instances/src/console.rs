//! Output del gioco. Con la configurazione log4j di Mojang il gioco scrive su
//! stdout eventi XML (`<log4j:Event …>`), pensati per essere letti da un
//! launcher: qui diventano righe strutturate, mostrabili da CLI e UI.

use std::fmt;

/// Un evento di log del gioco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEvent {
    pub level: String,
    pub thread: String,
    pub logger: String,
    /// Millisecondi dall'epoch Unix.
    pub timestamp_ms: Option<u64>,
    pub message: String,
    /// Stack trace dell'eccezione, se presente.
    pub throwable: Option<String>,
}

/// Una riga di output del gioco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameOutput {
    Event(LogEvent),
    /// Testo che non è un evento log4j (es. output della JVM).
    Raw(String),
}

impl fmt::Display for GameOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Raw(line) => f.write_str(line),
            Self::Event(e) => {
                if let Some(ms) = e.timestamp_ms {
                    let secs = (ms / 1000) % 86_400;
                    write!(
                        f,
                        "[{:02}:{:02}:{:02} UTC] ",
                        secs / 3600,
                        secs / 60 % 60,
                        secs % 60
                    )?;
                }
                write!(f, "[{}/{}]: {}", e.thread, e.level, e.message)?;
                if let Some(trace) = &e.throwable {
                    write!(f, "\n{}", trace.trim_end())?;
                }
                Ok(())
            }
        }
    }
}

/// Parser riga per riga: un evento log4j occupa più righe, quindi le righe
/// vengono accumulate fino a `</log4j:Event>`.
#[derive(Debug, Default)]
pub struct Log4jParser {
    pending: Option<String>,
}

impl Log4jParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fornisce una riga; restituisce un output quando una riga grezza o un
    /// evento completo è disponibile.
    pub fn push_line(&mut self, line: &str) -> Option<GameOutput> {
        match &mut self.pending {
            None if line.trim_start().starts_with("<log4j:Event") => {
                self.pending = Some(String::new());
                self.push_line(line)
            }
            None => Some(GameOutput::Raw(line.to_owned())),
            Some(buffer) => {
                buffer.push_str(line);
                buffer.push('\n');
                if !line.contains("</log4j:Event>") {
                    return None;
                }
                let event = self.pending.take().unwrap_or_default();
                Some(parse_event(&event).map_or(GameOutput::Raw(event), GameOutput::Event))
            }
        }
    }

    /// Restituisce l'eventuale evento incompleto (es. gioco terminato a metà).
    pub fn finish(&mut self) -> Option<GameOutput> {
        self.pending.take().map(GameOutput::Raw)
    }
}

fn parse_event(xml: &str) -> Option<LogEvent> {
    let open_end = xml.find('>')?;
    let open = &xml[..open_end];
    Some(LogEvent {
        level: attribute(open, "level")?,
        thread: attribute(open, "thread").unwrap_or_default(),
        logger: attribute(open, "logger").unwrap_or_default(),
        timestamp_ms: attribute(open, "timestamp").and_then(|t| t.parse().ok()),
        message: element_text(xml, "log4j:Message").unwrap_or_default(),
        throwable: element_text(xml, "log4j:Throwable"),
    })
}

/// Valore di `name="…"` nel tag di apertura, con le entità XML decodificate.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let len = tag[start..].find('"')?;
    Some(unescape(&tag[start..start + len]))
}

/// Testo di un elemento: concatena le sezioni CDATA (log4j spezza un
/// messaggio che contiene `]]>` in più sezioni) e decodifica il resto.
fn element_text(xml: &str, element: &str) -> Option<String> {
    let start = xml.find(&format!("<{element}>"))? + element.len() + 2;
    let len = xml[start..].find(&format!("</{element}>"))?;
    let mut rest = &xml[start..start + len];
    let mut text = String::new();
    while let Some(cdata) = rest.find("<![CDATA[") {
        text.push_str(&unescape(&rest[..cdata]));
        let body = &rest[cdata + 9..];
        let end = body.find("]]>").unwrap_or(body.len());
        text.push_str(&body[..end]);
        rest = body.get(end + 3..).unwrap_or("");
    }
    text.push_str(&unescape(rest));
    Some(text)
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_all(text: &str) -> Vec<GameOutput> {
        let mut parser = Log4jParser::new();
        let mut out: Vec<_> = text.lines().filter_map(|l| parser.push_line(l)).collect();
        out.extend(parser.finish());
        out
    }

    #[test]
    fn parses_real_minecraft_events() {
        // Formato reale osservato con Minecraft 26.3 e client-1.21.2.xml.
        let out = parse_all(
            r#"Picked up JAVA_TOOL_OPTIONS
<log4j:Event logger="net.minecraft.client.Minecraft" timestamp="1794854783000" level="INFO" thread="Render thread">
  <log4j:Message><![CDATA[Setting user: Dev]]></log4j:Message>
</log4j:Event>
<log4j:Event logger="x" timestamp="1794854784160" level="ERROR" thread="Worker &amp; Main-3">
  <log4j:Message><![CDATA[Failed: a ]]]]><![CDATA[> b]]></log4j:Message>
  <log4j:Throwable><![CDATA[java.lang.RuntimeException: boom
	at Foo.bar(Foo.java:1)
]]></log4j:Throwable>
</log4j:Event>"#,
        );
        assert_eq!(out.len(), 3);
        assert_eq!(
            out[0],
            GameOutput::Raw("Picked up JAVA_TOOL_OPTIONS".into())
        );

        let GameOutput::Event(user) = &out[1] else {
            panic!("{:?}", out[1]);
        };
        assert_eq!(user.message, "Setting user: Dev");
        assert_eq!(user.thread, "Render thread");
        assert_eq!(user.level, "INFO");
        assert_eq!(user.timestamp_ms, Some(1_794_854_783_000));
        assert!(
            out[1]
                .to_string()
                .ends_with("[Render thread/INFO]: Setting user: Dev")
        );

        let GameOutput::Event(error) = &out[2] else {
            panic!("{:?}", out[2]);
        };
        assert_eq!(error.thread, "Worker & Main-3");
        assert_eq!(error.message, "Failed: a ]]> b");
        assert!(error.throwable.as_deref().unwrap().contains("at Foo.bar"));
        assert!(out[2].to_string().contains("RuntimeException: boom"));
    }

    #[test]
    fn incomplete_event_is_returned_raw_at_the_end() {
        let out = parse_all("<log4j:Event level=\"INFO\" thread=\"t\">\n  <log4j:Message>");
        assert!(matches!(&out[..], [GameOutput::Raw(raw)] if raw.contains("log4j:Event")));
    }
}
