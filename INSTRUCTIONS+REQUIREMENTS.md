# Instructions and Requirements

I want to implmenet a ratatui terminal UI time tracking app in rust with the name "zeitlupius". The app must meet the following requirements: 

## General

- Ratatui terminal app
- cli for the app
- Time tracking for projects
- Projects are creatable and deletable in the app

## Projects

- Each project must be trackable independently
- Each project's times must be stored in a seperate file (time table, csv format)

### Time Tracking

- Tracking must be statable from inside the app OR via command in the terminal e.g. `zeitlupius <project> start`
- The start command in the app or the terminal should write the start time in the project time table 
- Tracking must be stoppable from inside the app OR via command in the terminal e.g. `zeitlupius <project> stop` 
- The stop command in the app or the terminal should write the stop time in the project time table
- It should be possible to start the timer for one project while another project has already started

## TUI (terminal user interface)

- The terminal ui should show a dashboard of all projects with their summed up times in specific time intervals
- Time intervals should be day, week, month, year
- The time intervals must be pageable. e.g.
    - Day: today, yesterday, the day before yesterday...
    - Week: the current calendar week, the last calendar week...
    - Month: the current calendar month, the last calendar month...
    - Year: the current calendar year, the last calendar year...
- It must be possible to query a custom time interval e.g. 18.04.2024 - 16.07.2024 (dd.mm.yyyy)
- The terminal ui should show a timer for each project that has been started

## CLI (command line interface)

- It should be possible to query time intervals via command line interface
    - Flag for time intervals like day, week, month, year
    - Possibility to enter custom time intervals
- It should be possible to list all projects 
- It should be possible to print current timer statable
- It should be possible to create and delete a project

The commands should be easy to understand and to use. All functions and features of the TUI must also be possible to do over the cli (and vice versa)

