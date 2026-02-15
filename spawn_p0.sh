#!/bin/bash

# P0 Tasks Parallel Execution Script

echo "Starting P0 Tasks..."

# Backend Task
bunx oh-my-ag agent:spawn backend .agent/tasks/task-01.md session-p0-backend -w . &
BACKEND_PID=$!
echo "Spawned Backend Agent (PID: $BACKEND_PID)"

# Frontend Task
bunx oh-my-ag agent:spawn frontend .agent/tasks/task-02.md session-p0-frontend -w . &
FRONTEND_PID=$!
echo "Spawned Frontend Agent (PID: $FRONTEND_PID)"

wait $BACKEND_PID $FRONTEND_PID
echo "P0 Tasks Completed."
