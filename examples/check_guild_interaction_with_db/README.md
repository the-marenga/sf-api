
# SFGAME Guild Event Notifier

## Overview

BEWARE: This code is simply my implementation of the sf-api in my application. I just want to provide possibilities and help anyone who would be struggling with this.

This application is designed to automate notifications for specific guild events in SFGAME. It targets characters on a designated server (Server 7) and monitors for upcoming guild-related events, particularly defense and attack dates. This tool aims to enhance the game experience by ensuring timely and efficient communication of event details via Discord notifications.

## Key Features

### SFGAME Account Interaction
- **Login and Access**: The app logs into an SFGAME account using provided credentials to access game-related data.

### Character Processing
- **Server-Specific Monitoring**: It processes characters associated with the logged-in account, identifying those located on the specified server (Server 7).

### Event Detection for Guild
- **Game State Retrieval**: For each character on the desired server, the app retrieves the game state, focusing on guild-related events.
- **Event Monitoring**: It specifically checks for upcoming defense and attack events.

### Event Notification Preparation
- **Event Message Construction**: Constructs detailed messages for each detected event (defense or attack), including the event type and date.

### Database Interaction for Discord Notification
- **Unique Notification Queueing**: Prior to notification dispatch, the app checks a MySQL database table (`discord_queue`) to avoid duplicate processing.
- **Database Insertion**: New messages are inserted into the `discord_queue` table with a 'pending' status, indicating they are queued for subsequent Discord notifications.

### Handling Non-occurrence of Events
- **Event Absence Logging**: In cases where no upcoming defense or attack events are found, the app logs this information without performing any database actions.

### Asynchronous Operations
- **Efficiency and Responsiveness**: The app performs operations asynchronously, allowing efficient handling of IO-bound tasks like network requests and database queries, ensuring non-blocking execution.

## Summary
This application serves as an automated notifier for specific guild events in SFGAME, efficiently handling the detection and notification of upcoming guild events. It employs asynchronous programming for optimal performance and integrates with a database to manage notification dispatch, streamlining communication within the SFGAME community.
