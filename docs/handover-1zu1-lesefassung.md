# Handover zum Vorlesen: Was für den kompletten Eins-zu-eins-Port noch fehlt

Stand: 18. November 2026, Branch Mango.

Ziel des Projekts: Das Programm OneDev, Version 17 Punkt 2 Punkt 1,
eins zu eins von Java nach Rust portieren. Kein abgespecktes
Minimalprodukt, sondern alles.

Der aktuelle Code kompiliert fehlerfrei, alle Tests sind grün.

## Was schon fertig ist

Erstens: Die Kerndatenbank und die Anmeldung. Benutzer, Projekte,
Rollen mit Berechtigungen, alles funktioniert und ist getestet.

Zweitens: Die Programmierschnittstelle. Alle Bereiche sind echt
verdrahtet: Anmeldung, Benutzer, Projekte, Gruppen, Aufgaben,
Zusammenführungsanfragen, Bauläufe, Git-Zugriff und große Dateien.
Keine Platzhalter mehr.

Drittens: Git. Repositories lesen, Zweige, Verlauf, Dateiinhalte,
Vergleiche, Schutzregeln und Code-Übernahme per Netzwerkprotokoll.

Viertens: Die Build-Maschine. Sie versteht echte OneDev-Bauvorschriften,
inklusive Variablen, Wiederholungsregeln und Vorlagen. Dazu ein
Programm zum Abholen von Container-Bildern aus Registries.
130 Tests sind grün.

Fünftens: Die Web-Oberfläche, Grundlagen. Das Programm startet,
die Anmeldeseite sieht aus wie im Original, ebenso das Grundgerüst
und fünf Listen: Projekte, Aufgaben, Zusammenführungsanfragen,
Bauläufe und Code. Alle Seiten laden und lassen sich durchsuchen,
mit echten Daten aus der Datenbank.

Sechstens: Das Startgerüst. Datenbank, Kernprogramm und Web-Oberfläche
lassen sich per Docker-Compose bauen und starten. Ein einziges Skript
namens docker-build steuert alles: bauen, starten, stoppen, neustarten,
aktualisieren, Protokolle ansehen. Ein zweites Skript namens temp-db
erzeugt eine Wegwerf-Datenbank zum Testen.

## Was noch fehlt, nach Wichtigkeit

### Höchste Priorität, ohne das geht es nicht weiter

Punkt eins: Die Web-Oberfläche ist erst zu einem kleinen Teil fertig.
Etwa 520 von 552 Seiten fehlen noch. Darunter die Detailansicht von
Aufgaben mit Kommentaren, die Detailansicht von Zusammenführungsanfragen
mit Code-Vergleich, die Build-Detailseite mit laufendem Protokoll,
der komplette Datei-Browser mit Vergleichsansicht, das Aufgaben-Board,
die Build-Grafik, alle Verwaltungsseiten und alle 429 JavaScript-Dateien
aus dem Original.

Punkt zwei: Die Suche fehlt komplett. Im Original sind das über
300 Klassen plus elf Grammatiken für Suchanfragen. Geplant ist ein
Ersatz mit den Rust-Werkzeugen Tantivy und Pest. Ohne Suche gibt es
keine echte Parität.

Punkt drei: Die Suchsprache fehlt. Alle Listen filtern derzeit nur
nach einfachen Texttreffern statt mit der echten Anfragesprache des
Originals.

### Hohe Priorität

Punkt vier: Bau-Ausführer. Nur der einfache Rechner-lokale Ausführungsmodus
existiert. Es fehlen Docker, Kubernetes, entfernte Rechner und
abgeschottete Umgebungen.

Punkt fünf: Berichts-Auswertungen. Etwa 25 Stück fehlen, zum Beispiel
für JUnit, Python-Prüfungen und Testabdeckung.

Punkt sechs: Paket-Verwaltung. Acht Paketformate fehlen, zum Beispiel
Maven, Npm, PyPI und NuGet. Nur die Datenmodelle existieren.

Punkt sieben: Schnittstellen-Lücken. Es fehlen die Endpunkte für Rollen
und Berechtigungen, für SSH-Schlüssel, Mitgliedschaften und Verwaltung.

Punkt acht: Anmelde-Restarbeiten. Die Passwort-Umstellung auf Argon
ist unvollständig, die Sitzungs-Erneuerung muss geprüft werden.

### Mittlere Priorität

Punkt neun: Git-Lücken. SSH-Zugang, Herunterladen als Zip-Archiv,
Datei-Filter und Hintergrundaufgaben.

Punkt zehn: Build-Lücken. Protokoll-Übertragung im Browser,
Artefakt-Browser, Parameter-Matrix.

Punkt elf: Das Kommandozeilen-Werkzeug enthält nur Platzhalter,
keine echte Steuerung.

Punkt zwölf: Tests. Es fehlen Tests für Schutzregeln, Datei-Uploads
und Ende-zu-Ende-Abläufe.

### Organisatorisches

Punkt dreizehn: Der Docker-Bau läuft auf deinem Rechner, nicht hier,
weil es hier keinen Docker-Dienst gibt. Zwei deiner lokalen Änderungen
sind bewusst nicht eingespielt: die SupaDB-Datenbank in der
Compose-Datei und eine kleine Änderung an der Ignore-Datei.

Punkt vierzehn: Eine automatische Prüfpipeline für den Rust-Code
gibt es noch nicht.

## Empfohlene Reihenfolge

Erstens: die Suche bauen, denn sie entsperrt alle Listen.
Zweitens: die Aufgaben-Detailseite nach dem Original nachbauen.
Drittens: die Zusammenführungs-Detailseite mit Code-Vergleich.
Viertens: den Datei-Browser vervollständigen.
Fünftens: den Docker-Ausführer plus die erste Berichts-Auswertung.
Sechstens: die Rollen- und Berechtigungs-Schnittstellen.
Siebtens: das Kommandozeilen-Werkzeug echt verdrahten.
Achtens: die Build-Detailseite mit laufendem Protokoll.

## Offene Dateien auf deinem Rechner, bewusst nicht angerührt

Die SupaDB-Umstellung in der Compose-Datei. Deine Ignore-Datei-Änderung.
Die Tondatei user-report-punkt-em-pee-drei. Die Geheimnisse unter
docker-slash-secrets. Sie sind ignoriert und werden nicht hochgeladen.

Ende des Handovers.
