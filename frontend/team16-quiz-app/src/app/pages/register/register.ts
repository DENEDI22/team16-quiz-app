import {Component, inject, signal} from '@angular/core';
import {ReactiveFormsModule, FormBuilder, Validators} from '@angular/forms';
import {RouterLink} from '@angular/router';
import {MatCardModule} from '@angular/material/card';
import {MatFormFieldModule} from '@angular/material/form-field';
import {MatInputModule} from '@angular/material/input';
import {MatButtonModule} from '@angular/material/button';
import {MatProgressSpinnerModule} from '@angular/material/progress-spinner';
import {AuthService} from '../../services/auth/auth';

@Component({
  selector: 'app-register',
  standalone: true,
  imports: [
    ReactiveFormsModule,
    MatCardModule,
    MatFormFieldModule,
    MatInputModule,
    MatButtonModule,
    MatProgressSpinnerModule,
    RouterLink
  ],
  templateUrl: './register.html',
  styleUrl: './register.scss'
})
export class Register {
  isLoading = signal(false);
  errorMessage = signal('');
  successMessage = signal('');
  private authService = inject(AuthService);
  registerForm;

  constructor(private fb: FormBuilder) {
    this.registerForm = this.fb.group({
      username: ['', [Validators.required, Validators.minLength(3)]],
      email: ['', [Validators.required, Validators.email]],
      password: ['', [Validators.required, Validators.minLength(6)]],
    });
  }

  onSubmit() {
    this.errorMessage.set('');
    this.successMessage.set('');

    if (this.registerForm.invalid) {
      this.registerForm.markAllAsTouched();
      return;
    }

    this.isLoading.set(true);

    const formValue = this.registerForm.getRawValue();

    this.authService.register({
      username: formValue.username!,
      email: formValue.email!,
      password: formValue.password!,
    }).subscribe({
      next: (response) => {
        this.isLoading.set(false);

        if (response.token) {
          localStorage.setItem('token', response.token);
        }

        this.successMessage.set('Registrierung erfolgreich.');
      },
      error: (error) => {
        this.isLoading.set(false);

        const message =
          error.error?.data?.message || 'Registrierung fehlgeschlagen.';

        this.errorMessage.set(message);
      }
    });
  }
}
