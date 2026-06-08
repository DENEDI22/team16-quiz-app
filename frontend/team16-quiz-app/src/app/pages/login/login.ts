import { Component, inject, signal } from '@angular/core';
import { ReactiveFormsModule, FormBuilder, Validators } from '@angular/forms';
import { Router, RouterLink } from '@angular/router';

import { MatCardModule } from '@angular/material/card';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatButtonModule } from '@angular/material/button';

import { AuthService } from '../../auth/services/auth';

@Component({
  selector: 'app-login',
  standalone: true,
  imports: [
    ReactiveFormsModule,
    MatCardModule,
    MatFormFieldModule,
    MatInputModule,
    MatButtonModule,
    RouterLink
  ],
  templateUrl: './login.html',
  styleUrl: './login.scss'
})
export class Login {
  private formBuilder = inject(FormBuilder);
  private authService = inject(AuthService);
  private router = inject(Router);

  isLoading = signal(false);
  errorMessage = signal('');

  loginForm = this.formBuilder.group({
    email: ['', [Validators.required, Validators.email]],
    password: ['', [Validators.required, Validators.minLength(6)]],
  });

  onSubmit() {
    console.log('Login submit clicked');
    this.errorMessage.set('');

    if (this.loginForm.invalid) {
      this.loginForm.markAllAsTouched();
      return;
    }

    this.isLoading.set(true);

    const formValue = this.loginForm.getRawValue();

    this.authService.login({
      email: formValue.email!,
      password: formValue.password!,
    }).subscribe({
      next: (response) => {
        console.log('Login response:', response);

        this.isLoading.set(false);

        const token = response.token || response.data?.message;

        if (token) {
          localStorage.setItem('token', token);
          this.router.navigate(['/']);
        }
      },
      error: (error) => {
        console.log('Login error:', error);

        this.isLoading.set(false);

        const message =
          error.error?.data?.message || 'Login fehlgeschlagen.';

        this.errorMessage.set(message);
      }
    });
  }
}
